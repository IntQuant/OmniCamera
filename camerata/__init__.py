from dataclasses import dataclass
from . import camerata
import sys
try:
    import numpy as np
except ImportError:
    print("[Camerata] Could not import numpy", file=sys.stderr)

@dataclass
class CameraInfo:
    """
    Describes a connected camera.
    """
    index: int
    name: str
    description: str
    misc: str

    def can_open(self):
        """
        Check if this camera can be opened.
        """
        return camerata.check_can_use(self.index)


class Camera:
    def __init__(self, info: CameraInfo, suggested_fps: int = 25):
        """
        Open a camera corresponding to the info object.
        Will try to use maximum possible resolution with a frame rate of at least *suggested_fps*
        """
        self.info = info
        self._cam = camerata.Camera(info.index, suggested_fps)
    
    def poll_frame_raw(self) -> tuple[int, int, bytes]:
        """
        Get a frame from the camera. Returns width, height, and array of raw rgb values.
        Guaranteed to never block, but may return zero-filled frames if no frames were recieved from camera yet.
        """
        return self._cam.poll_frame()

    def poll_frame_np(self) -> "np.ndarray":
        """
        Get a frame from the camera. Returns a numpy array.
        Guaranteed to never block, but may return zero-filled frames if no frames were recieved from camera yet.
        """
        w, h, data = self.poll_frame_raw()
        shape = (w, h, 3)
        arr = np.frombuffer(data, dtype=np.uint8)
        return arr.reshape(shape)
        

def query(only_usable=True) -> list[CameraInfo]:
    """
    Returns a list of CameraInfo objects, one for every available camera.
    """
    result = map(lambda x: CameraInfo(*x), camerata.query())
    if only_usable:
        result = filter(CameraInfo.can_open, result)
    return list(result)

