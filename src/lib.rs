
use std::{cell::RefCell, sync::{atomic, Arc}};

use cpython::{py_module_initializer, py_class, PyResult, exc, PyErr, ToPyObject, PythonObject, PyBytes, py_fn, Python};
use nokhwa::CameraFormat;

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

fn get_compatible_format(cam: &mut nokhwa::ThreadedCamera, suggested_fps: u32) -> Result<CameraFormat, Box<dyn std::error::Error>> {
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

impl  Camera {

}

py_class!(class Camera |_py| {
    data cam: RefCell<nokhwa::ThreadedCamera>;
    data has_captured: Arc<atomic::AtomicBool>;

    def __new__(_cls, index: usize, suggested_fps: u32) -> PyResult<Camera> {
        match nokhwa::ThreadedCamera::new(index, None) {
            Ok(mut cam) => {
                let format = get_compatible_format(&mut cam, suggested_fps).unwrap();
                //eprintln!("Selected format: {:?}", format);
                cam.set_camera_format(format).unwrap();
                let has_captured = Arc::new(atomic::AtomicBool::new(false));
                let _has_captured_clone = Arc::clone(&has_captured);
                cam.open_stream(|_| {}).unwrap();
                Camera::create_instance(_py, RefCell::new(cam), has_captured)
            },
            Err(error) => {
                Err(PyErr::new_lazy_init(_py.get_type::<exc::RuntimeError>(), Some(error.to_string().into_py_object(_py).into_object())))
            }
        }
    }

    def poll_frame(&self) -> PyResult<(u32, u32, PyBytes)> {
        let frame = self.cam(_py).borrow_mut().last_frame();
        Ok((frame.width(), frame.height(), PyBytes::new(_py, &frame)))
    }
});
