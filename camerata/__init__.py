from dataclasses import dataclass
from typing import Union
from . import camerata
import sys
try:
    import numpy as np
except ImportError:
    print("[Camerata] Could not import numpy", file=sys.stderr)

try:
    from PIL import Image
except ImportError:
    print("[Camerata] Could not import pillow", file=sys.stderr)

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
    
    def poll_frame_raw(self) -> Union[tuple[int, int, bytes], None]:
        """
        Get a frame from the camera. Returns width, height, and array of raw rgb values.
        Guaranteed to never block, but may return None if no frames were recieved from camera yet.
        """
        return self._cam.poll_frame()

    def poll_frame_np(self) -> Union["np.ndarray", None]:
        """
        Get a frame from the camera. Returns a numpy array.
        Guaranteed to never block, but may return None if no frames were recieved from camera yet.
        """
        frame = self.poll_frame_raw()
        if frame is None:
            return None
        w, h, data = frame
        shape = (h, w, 3)
        arr = np.frombuffer(data, dtype=np.uint8)
        return arr.reshape(shape)
    
    def poll_frame_pil(self) -> Union["Image.Image", None]:
        frame = self.poll_frame_raw()
        if frame is None:
            return None
        return Image.frombytes("RGB", (frame[0], frame[1]), frame[2])
    
    def _info(self):
        return self._cam.info()


def query(only_usable=True) -> list[CameraInfo]:
    """
    Returns a list of CameraInfo objects, one for every available camera.
    """
    result = map(lambda x: CameraInfo(*x), camerata.query())
    if only_usable:
        result = filter(CameraInfo.can_open, result)
    return list(result)

