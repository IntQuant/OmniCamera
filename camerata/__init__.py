from dataclasses import dataclass
from enum import Enum
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


class FrameFormat(Enum):
    MJPEG = "mjpeg"
    YUYV = "yuyv"


class CameraFormat:
    def __init__(self, cam_format: camerata.CamFormat):
        self._fmt = cam_format
    
    @property
    def width(self) -> int:
        return self._fmt.width

    @property
    def height(self) -> int:
        return self._fmt.height

    @property
    def frame_rate(self) -> int:
        return self._fmt.frame_rate

    @property
    def frame_format(self) -> FrameFormat:
        return FrameFormat(self._fmt.format)
    
    def __str__(self) -> str:
        return f"{self.frame_format.value} {self.width}x{self.height}@{self.frame_rate}fps"


class CameraFormatOptions(list):
    """
    A list of camera formats with additional methods to choose the most fitting camera format.
    prefer* functions either return only fitting formats or everything, in case there are no fitting formats.
    """

    def prefer(self, func):
        """
        Prefer formats for which func is true.
        """
        options = CameraFormatOptions(
            filter(func, self)
            )
        if not options:
            return self
        return options

    def prefer_fps_range(self, min_fps: int=None, max_fps: int=None):
        """
        Prefer formats with min_fps <= frame_rate <= max_fps.
        """
        return self.prefer(lambda x: (min_fps is None or x.frame_rate>=min_fps) and (max_fps is None or x.frame_rate<=max_fps))

    def prefer_sides_ratio(self, width_by_height: float):
        """
        Prefer formats with width / height == width_by_height
        """
        return self.prefer(lambda x: abs(x.width/x.height-width_by_height) < 1e-6)

    def prefer_frame_format(self, fmt: FrameFormat):
        return self.prefer(lambda x: x.frame_format is fmt)

    def resolve(self) -> CameraFormat:
        """
        Returns one of contained formats.
        """
        self.sort(key=lambda x:x.width)
        return self[-1]

    def resolve_default(self) -> CameraFormat:
        """
        Like resolve, but applies some default preferences.
        Used when camera is opened automatically.
        """
        return self.prefer_fps_range(25, 60).prefer_sides_ratio(4/3).prefer_frame_format(FrameFormat.MJPEG).resolve()

class Camera:
    def __init__(self, info: CameraInfo, suggested_fps: int = 25):
        """
        Open a camera corresponding to the info object.
        Will try to use maximum possible resolution with a frame rate of at least *suggested_fps*
        """
        self.info = info
        self._initialized = False
        self._cam = camerata.Camera(info.index)
    
    def get_format_options(self) -> CameraFormatOptions:
        """
        Returns a list of supported CameraFormat objects.
        """
        return CameraFormatOptions(map(CameraFormat, self._cam.get_formats()))

    def open(self, fmt: CameraFormat = None):
        """
        Select a format and open the camera.
        Called automatically if needed from poll_frame_* method family.
        """
        if self._initialized:
            raise RuntimeError("Can only open once")
        if fmt is None:
            fmt = self.get_format_options().resolve_default()
        self._cam.open(fmt._fmt)
        self._initialized = True

    def poll_frame_raw(self) -> Union[tuple[int, int, bytes], None]:
        """
        Get a frame from the camera. Returns width, height, and array of raw rgb values.
        Guaranteed to never block, but may return None if no frames were recieved from camera yet.
        """
        if not self._initialized:
            self.open()
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

