use std::{
    mem,
    sync::{atomic, Arc, Mutex, Weak},
};

use image::{ImageBuffer, Rgb};
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{
        ApiBackend, CameraControl, CameraFormat, CameraIndex, ControlValueDescription, FrameFormat,
        RequestedFormat, RequestedFormatType,
    },
};
use parking_lot::FairMutex;
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    types::PyBytes,
};

#[pyfunction]
pub fn query() -> PyResult<Vec<(u32, String, String, String)>> {
    let devices = match nokhwa::query(ApiBackend::Auto) {
        Ok(val) => val,
        Err(error) => return Err(PyRuntimeError::new_err(error.to_string())),
    };
    let mut result = Vec::with_capacity(devices.len());
    for device in devices.into_iter() {
        if let CameraIndex::Index(index) = *device.index() {
            result.push((
                index,
                device.human_name(),
                device.description().to_owned(),
                device.misc(),
            ));
        }
    }
    Ok(result)
}

#[pyfunction]
pub fn check_can_use(index: u32) -> PyResult<bool> {
    Ok(nokhwa::Camera::new(
        CameraIndex::Index(index),
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::None),
    )
    .is_ok())
}

#[pymodule]
fn omni_camera(_py: Python, m: &PyModule) -> PyResult<()> {
    nokhwa::nokhwa_initialize(|_| {});
    m.add_function(wrap_pyfunction!(query, m)?)?;
    m.add_function(wrap_pyfunction!(check_can_use, m)?)?;
    m.add_class::<Camera>()?;
    m.add_class::<CamFormat>()?;
    m.add_class::<CamControl>()?;
    Ok(())
}

type Image = ImageBuffer<Rgb<u8>, Vec<u8>>;

struct CameraInternal {
    camera: Arc<FairMutex<nokhwa::Camera>>,
    active: Arc<atomic::AtomicBool>,
    last_frame: Arc<FairMutex<Arc<Option<Image>>>>,
    last_err: Arc<FairMutex<Option<nokhwa::NokhwaError>>>,
}

impl CameraInternal {
    fn new(cam: nokhwa::Camera) -> CameraInternal {
        CameraInternal {
            camera: Arc::new(FairMutex::new(cam)),
            active: Arc::new(atomic::AtomicBool::new(true)),
            last_frame: Arc::new(FairMutex::new(Arc::new(None))),
            last_err: Arc::new(FairMutex::new(None)),
        }
    }
    fn start(&self, format: CameraFormat) -> Result<(), nokhwa::NokhwaError> {
        let active = Arc::clone(&self.active);
        let last_frame = Arc::clone(&self.last_frame);
        let camera = Arc::clone(&self.camera);
        let last_err = Arc::clone(&self.last_err);
        std::thread::spawn(move || {
            let mut cam_guard = camera.lock();
            if let Err(err) = cam_guard
                .set_camera_format(format)
                .and(cam_guard.open_stream())
            {
                *last_err.lock() = Some(err);
                return;
            }
            mem::drop(cam_guard);
            while active.load(atomic::Ordering::Relaxed) {
                if let Ok(frame) = camera.lock().frame() {
                    *last_frame.lock() = Arc::new(frame.decode_image::<RgbFormat>().ok());
                }
            }
        });
        Ok(())
    }
    fn last_frame(&self) -> Arc<Option<ImageBuffer<Rgb<u8>, Vec<u8>>>> {
        Arc::clone(&self.last_frame.lock())
    }
}

impl Drop for CameraInternal {
    fn drop(&mut self) {
        self.active.store(false, atomic::Ordering::Relaxed);
    }
}

#[derive(Clone)]
#[pyclass]
struct CamFormat {
    #[pyo3(get)]
    width: u32,
    #[pyo3(get)]
    height: u32,
    #[pyo3(get)]
    frame_rate: u32,
    format: FrameFormat,
}

#[pymethods]
impl CamFormat {
    #[getter]
    fn get_format(&self) -> String {
        match self.format {
            FrameFormat::MJPEG => "mjpeg".to_string(),
            FrameFormat::YUYV => "yuyv".to_string(),
            FrameFormat::GRAY => "gray".to_string(),
            FrameFormat::NV12 => "nv12".to_string(),
            FrameFormat::RAWRGB => "rawrgb".to_string(),
        }
    }
    //#[setter]
    fn set_format(&mut self, fmt: String) -> PyResult<()> {
        self.format = match fmt.as_str() {
            "mjpeg" => FrameFormat::MJPEG,
            "yuyv" => FrameFormat::YUYV,
            _ => {
                return Err(PyValueError::new_err(
                    "Unsupported value (should be one of 'mjpeg', 'yuyv')",
                ))
            }
        };
        Ok(())
    }
}

impl From<CamFormat> for CameraFormat {
    fn from(fmt: CamFormat) -> CameraFormat {
        CameraFormat::new_from(fmt.width, fmt.height, fmt.format, fmt.frame_rate)
    }
}

impl From<CameraFormat> for CamFormat {
    fn from(fmt: CameraFormat) -> Self {
        CamFormat {
            width: fmt.width(),
            height: fmt.height(),
            format: fmt.format(),
            frame_rate: fmt.frame_rate(),
        }
    }
}

#[pyclass]
struct CamControl {
    cam: Weak<FairMutex<nokhwa::Camera>>,
    control: Mutex<CameraControl>,
}

#[pymethods]
impl CamControl {
    fn value_range(&self) -> (i64, i64, i64) {
        let control = self.control.lock().unwrap();
        let control_desc = control.description();
        match control_desc {
            ControlValueDescription::IntegerRange { min, max, step, .. } => (*min, *max, *step),
            _ => todo!(),
        }
    }
    fn set_value(&self, value: Option<i64>) -> PyResult<()> {
        let mut control = self.control.lock().unwrap();
        match self.cam.upgrade() {
            Some(cam) => match value {
                Some(value) => {
                    control.set_active(true);
                    let mut cam = cam.lock();
                    match cam.set_camera_control(
                        control.control(),
                        nokhwa::utils::ControlValueSetter::Integer(value),
                    ) {
                        Ok(_) => Ok(()),
                        Err(error) => Err(PyRuntimeError::new_err(error.to_string())),
                    }
                }
                None => {
                    control.set_active(false);
                    Ok(())
                }
            },
            None => Err(PyRuntimeError::new_err(
                "Control is unusable as camera object has been dropped".to_string(),
            )),
        }
    }
}

#[pyclass]
struct Camera {
    cam: CameraInternal,
}

#[pymethods]
impl Camera {
    #[new]
    fn new(index: u32) -> PyResult<Camera> {
        match nokhwa::Camera::new(
            CameraIndex::Index(index),
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::None),
        ) {
            Ok(cam) => Ok(Camera {
                cam: CameraInternal::new(cam),
            }),
            Err(error) => Err(PyRuntimeError::new_err(error.to_string())),
        }
    }
    fn open(&self, format: CamFormat) -> PyResult<()> {
        if let Err(error) = self.cam.start(format.into()) {
            return Err(PyRuntimeError::new_err(error.to_string()));
        }
        let has_captured = Arc::new(atomic::AtomicBool::new(false));
        let _has_captured_clone = Arc::clone(&has_captured);
        Ok(())
    }

    fn info(&self) -> PyResult<String> {
        Ok(format!(
            "Selected format: {:?}",
            self.cam.camera.lock().camera_format()
        ))
    }

    fn get_formats(&self) -> PyResult<Vec<CamFormat>> {
        match self.cam.camera.lock().compatible_camera_formats() {
            Ok(formats) => Ok(formats.into_iter().map(|x| x.into()).collect()),
            Err(error) => Err(PyRuntimeError::new_err(error.to_string())),
        }
    }

    fn poll_frame(&self, py: Python) -> PyResult<Option<(u32, u32, Py<PyBytes>)>> {
        match &*self.cam.last_frame() {
            Some(frame) => Ok(Some((
                frame.width(),
                frame.height(),
                PyBytes::new(py, frame).into(),
            ))),
            None => Ok(None),
        }
    }

    fn check_err(&self) -> PyResult<()> {
        match &*self.cam.last_err.lock() {
            Some(error) => Err(PyRuntimeError::new_err(error.to_string())),
            None => Ok(()),
        }
    }
    fn get_controls(&self) -> PyResult<Vec<(String, CamControl)>> {
        match self.cam.camera.lock().camera_controls_string() {
            Ok(list) => Ok(list
                .into_iter()
                .map(|(name, control)| {
                    (
                        name,
                        CamControl {
                            control: Mutex::new(control),
                            cam: Arc::downgrade(&self.cam.camera),
                        },
                    )
                })
                .collect()),
            Err(_err) => {
                Ok(Vec::new()) // Nothing supported
            }
        }
    }
}
