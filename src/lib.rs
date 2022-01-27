
use std::sync::{atomic, Arc, Mutex};

use cpython::{py_module_initializer, py_class, PyResult, exc, PyErr, ToPyObject, PythonObject, PyBytes, py_fn, Python};
use image::{ImageBuffer, Rgb};
use nokhwa::CameraFormat;
use parking_lot::FairMutex;

py_module_initializer!(camerata, |py, m| {
    nokhwa::nokhwa_initialize(|_|{});
    m.add(py, "__doc__", "Module documentation string")?;
    m.add(py, "query", py_fn!(py, query()))?;
    m.add(py, "check_can_use", py_fn!(py, check_can_use(index: usize)))?;
    m.add_class::<Camera>(py)?;
    Ok(())
});

pub fn query(_py: Python) -> PyResult<Vec<(usize, String, String, String)>> {
    let devices = nokhwa::query().unwrap();
    let mut result = Vec::with_capacity(devices.len());
    for device in devices.into_iter() {
        result.push((device.index(), device.human_name(), device.description(), device.misc()));
    }
    Ok(result)
}

pub fn check_can_use(_py: Python, index: usize) -> PyResult<bool> {
    Ok(nokhwa::Camera::new(index, None).is_ok())
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

fn error_to_exception(py: Python, message: &str) -> PyErr {
    PyErr::new_lazy_init(py.get_type::<exc::RuntimeError>(), Some(message.into_py_object(py).into_object()))
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
        let active = Arc::clone(&me.active);
        let last_frame = Arc::clone(&me.last_frame);
        let camera = Arc::clone(&me.camera);
        std::thread::spawn(move || {
            while active.load(atomic::Ordering::Relaxed) {
                if let Ok(frame) = camera.lock().unwrap().frame() {
                    *last_frame.lock() = Arc::new(Some(frame));
                }
            }
        });
        me
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

py_class!(class Camera |_py| {
    data cam: CameraInternal;

    def __new__(_cls, index: usize, suggested_fps: u32) -> PyResult<Camera> {
        match nokhwa::Camera::new(index, None) {
            Ok(mut cam) => {
                let format = get_compatible_format(&mut cam, suggested_fps).unwrap();
                //eprintln!("Selected format: {:?}", format);
                cam.set_camera_format(format).unwrap();
                let has_captured = Arc::new(atomic::AtomicBool::new(false));
                let _has_captured_clone = Arc::clone(&has_captured);
                if let Err(error) = cam.open_stream() {
                    return Err(error_to_exception(_py, &error.to_string()));    
                }
                Camera::create_instance(_py, CameraInternal::new(cam))
            },
            Err(error) => {
                Err(error_to_exception(_py, &error.to_string()))
            }
        }
    }
    def info(&self) -> PyResult<String> {
        Ok(format!("Selected format: {:?}", self.cam(_py).camera.lock().unwrap().camera_format()))
    }

    def poll_frame(&self) -> PyResult<Option<(u32, u32, PyBytes)>> {
        match &*self.cam(_py).last_frame() {
            Some(frame) => {
                Ok(Some((frame.width(), frame.height(), PyBytes::new(_py, &frame))))
            },
            None => {
                Ok(None)
            }
        }
    }
});
