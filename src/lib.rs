
use std::{sync::{atomic, Arc, Mutex}, mem};

use image::{ImageBuffer, Rgb};
use pyo3::{prelude::*, exceptions::{PyRuntimeError, PyValueError}, types::PyBytes};
use nokhwa::{CameraFormat, FrameFormat};
use parking_lot::FairMutex;


#[pyfunction]
pub fn query() -> PyResult<Vec<(usize, String, String, String)>> {
    let devices = nokhwa::query().unwrap();
    let mut result = Vec::with_capacity(devices.len());
    for device in devices.into_iter() {
        result.push((device.index(), device.human_name(), device.description(), device.misc()));
    }
    Ok(result)
}

#[pyfunction]
pub fn check_can_use(index: usize) -> PyResult<bool> {
    Ok(nokhwa::Camera::new(index, None).is_ok())
}

#[pymodule]
fn camerata(_py: Python, m: &PyModule) -> PyResult<()> {
    nokhwa::nokhwa_initialize(|_|{});
    m.add_function(wrap_pyfunction!(query, m)?)?;
    m.add_function(wrap_pyfunction!(check_can_use, m)?)?;
    m.add_class::<Camera>()?;
    m.add_class::<CamFormat>()?;
    Ok(())
}

fn get_compatible_format(cam: &mut nokhwa::Camera, suggested_fps: u32) -> Result<CameraFormat, Box<dyn std::error::Error>> {
    let fourcc_s = cam.compatible_fourcc()?;
    //eprintln!("Supported fourccs: {:?}", fourcc_s);
    let map = cam.compatible_list_by_resolution(fourcc_s[0])?;
    let format = map.into_iter().filter_map(|(res, fps_vec)| {
        if let Some(fps) = fps_vec.into_iter().max() {
            Some(CameraFormat::new(res, fourcc_s[0], fps))
        } else {
            None
        }
    }).reduce(|acc, cur| {
        let enough_fps = u32::min(acc.frame_rate(), cur.frame_rate()) >= suggested_fps;
        let cond = if enough_fps {
            acc.width() > cur.width()
        } else {
            acc.frame_rate() > cur.frame_rate()
        };

        if cond {
            acc
        } else {
            cur
        }
    });
    match format {
        Some(format) => {
            Ok(format)
        },
        None => {
            Ok(cam.camera_format())
        }
    }
}

struct CameraInternal {
    camera: Arc<Mutex<nokhwa::Camera>>,
    active: Arc<atomic::AtomicBool>,
    last_frame: Arc<FairMutex<Arc<Option<ImageBuffer<Rgb<u8>, Vec<u8>>>>>>
}

impl CameraInternal {
    fn new(cam: nokhwa::Camera) -> CameraInternal {
        let me = CameraInternal { 
            camera: Arc::new(Mutex::new(cam)),
            active: Arc::new(atomic::AtomicBool::new(true)),
            last_frame: Arc::new(FairMutex::new(Arc::new(None))),
        };
        me
    }
    fn start(&self, format: CameraFormat) -> Result<(), nokhwa::NokhwaError>{
        
        let mut cam_guard = self.camera.lock().unwrap();
        cam_guard.set_camera_format(format)?;
        cam_guard.open_stream()?;
        mem::drop(cam_guard);
        let active = Arc::clone(&self.active);
        let last_frame = Arc::clone(&self.last_frame);
        let camera = Arc::clone(&self.camera);
        std::thread::spawn(move || {
            while active.load(atomic::Ordering::Relaxed) {
                if let Ok(frame) = camera.lock().unwrap().frame() {
                    *last_frame.lock() = Arc::new(Some(frame));
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
        }
    }
    //#[setter]
    fn set_format(&mut self, fmt: String) -> PyResult<()> {
        self.format = match fmt.as_str() {
            "mjpeg" => FrameFormat::MJPEG,
            "yuyv" => FrameFormat::YUYV,
            _ => return Err(PyValueError::new_err("Unsupported value (should be one of 'mjpeg', 'yuyv')")),
        };
        Ok(())
    }

}

impl Into<CameraFormat> for CamFormat {
    fn into(self) -> CameraFormat {
        CameraFormat::new_from(self.width, self.height, self.format, self.frame_rate)
    }
}

impl From<CameraFormat> for CamFormat {
    fn from(fmt: CameraFormat) -> Self {
        CamFormat { width: fmt.width(), height: fmt.height(), format: fmt.format(), frame_rate: fmt.frame_rate() }
    }
}

#[pyclass]
struct Camera {
    cam: CameraInternal,
}

#[pymethods]
impl Camera {
    #[new]
    fn new(index: usize) -> PyResult<Camera> {
        match nokhwa::Camera::new(index, None) {
            Ok(cam) => {
                //let format = get_compatible_format(&mut cam, suggested_fps).unwrap();
                //eprintln!("Selected format: {:?}", format);
                
                Ok(Camera{ cam: CameraInternal::new(cam) })
            },
            Err(error) => {
                Err(PyRuntimeError::new_err(error.to_string()))
            }
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
        Ok(format!("Selected format: {:?}", self.cam.camera.lock().unwrap().camera_format()))
    }

    fn get_formats(&self) -> PyResult<Vec<CamFormat>> {
        match self.cam.camera.lock().unwrap().compatible_camera_formats() {
            Ok(formats) => {
                Ok(formats.into_iter().map(|x| {x.into()}).collect())
            },
            Err(error) => {
                Err(PyRuntimeError::new_err(error.to_string()))
            }
        }
    }

    fn poll_frame(&self, py: Python) -> PyResult<Option<(u32, u32, Py<PyBytes>)>> {
        match &*self.cam.last_frame() {
            Some(frame) => {
                Ok(Some((frame.width(), frame.height(), PyBytes::new(py, &frame).into())))
            },
            None => {
                Ok(None)
            }
        }
    }
}