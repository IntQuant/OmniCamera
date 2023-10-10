from dataclasses import dataclass
from enum import Enum
from typing import Dict, List, Union
import warnings
from . import omni_camera
import sys
try:
    import numpy as np
except ImportError:
    print("[OmniCamera] Could not import numpy", file=sys.stderr)

try:
    from PIL import Image
except ImportError:
    print("[OmniCamera] Could not import pillow", file=sys.stderr)

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
        return omni_camera.check_can_use(self.index)


class FrameFormat(Enum):
    MJPEG = "mjpeg"
    YUYV = "yuyv"


class CameraFormat:
    def __init__(self, cam_format: omni_camera.CamFormat):
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

    def _prefer_range(self, val_getter, min_val=None, max_val=None):
        return self.prefer(lambda x: (min_val is None or val_getter(x)>=min_val) and (max_val is None or val_getter(x)<=max_val))
    
    def prefer_fps_range(self, min_fps: int=None, max_fps: int=None):
        """
        Prefer formats with min_fps <= frame_rate <= max_fps.
        """
        return self._prefer_range(lambda x: x.frame_rate, min_fps, max_fps)

    def prefer_width_range(self, min_width: int=None, max_width: int=None):
        """
        Prefer formats with min_width <= width <= max_width.
        """
        return self._prefer_range(lambda x: x.width, min_width, max_width)
    
    def prefer_height_range(self, min_heigth: int=None, max_height: int=None):
        """
        Prefer formats with min_heigth <= height <= max_height.
        """
        return self._prefer_range(lambda x: x.height, min_heigth, max_height)

    def prefer_aspect_ratio(self, width_by_height: float):
        """
        Prefer formats with width / height == width_by_height
        """
        return self.prefer(lambda x: abs(x.width/x.height-width_by_height) < 1e-6)

    def prefer_sides_ratio(self, width_by_height: float):
        warnings.warn("prefer_sides_ratio has been renamed to prefer_aspect_ratio", DeprecationWarning)
        return self.prefer_aspect_ratio(width_by_height)

    def prefer_frame_format(self, fmt: FrameFormat):
        return self.prefer(lambda x: x.frame_format is fmt)
    

    def resolve(self, key=lambda x: x.width) -> CameraFormat:
        """
        Returns one of contained formats.
        """
        return max(self, key=key)

    def resolve_default(self) -> CameraFormat:
        """
        Like resolve, but applies some default preferences.
        Used when camera is opened automatically.
        """
        return self.prefer_fps_range(25, 60).prefer_sides_ratio(4/3).prefer_frame_format(FrameFormat.MJPEG).resolve()


class CameraControl:
    def __init__(self, control):
        self._control = control
    
    @property
    def value_range(self) -> range:
        """
        Return a range of values that can be passed to set_value.
        """
        start, stop, step = self._control.value_range()
        # Just in case - ensure that start is divisible by step, as required by nokhwa
        new_start = start//step*step
        while new_start <= start:
            new_start += step
        return range(start, stop, step)

    def set_value(self, value: Union[int, None]):
        """
        Set a value for this control.
        *value* in self.value_range should be true.
        """
        if value is not None:
            assert value in self.value_range
        self._control.set_value(value)

    def set_fraction(self, fraction: float):
        """
        Set a value for this control.
        0 <= *fraction* <= 1 should be true.
        """
        assert 0 <= fraction <= 1
        ind = round(fraction * (len(self.value_range)-1))
        self.set_value(self.value_range[ind])


class Camera:
    def __init__(self, info: CameraInfo, suggested_fps: int = 25):
        """
        Open a camera corresponding to the info object.
        Will try to use maximum possible resolution with a frame rate of at least *suggested_fps*
        """
        self.info = info
        self._initialized = False
        self._cam = omni_camera.Camera(info.index)
    
    def get_format_options(self) -> CameraFormatOptions:
        """
        Returns a list of supported CameraFormat objects.
        """
        return CameraFormatOptions(map(CameraFormat, self._cam.get_formats()))

    def get_controls(self) -> Dict[str, CameraControl]:
        """
        [Experimental]
        Get a list of supported camera controls
        """
        return {k: CameraControl(v) for k, v in self._cam.get_controls()}

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
        Guaranteed to never block, but may return None if no frames were received from camera yet.
        """
        if not self._initialized:
            self.open()
        self._cam.check_err()
        return self._cam.poll_frame()

    def poll_frame_np(self) -> Union["np.ndarray", None]:
        """
        Get a frame from the camera. Returns a numpy array.
        Guaranteed to never block, but may return None if no frames were received from camera yet.
        """
        frame = self.poll_frame_raw()
        if frame is None:
            return None
        w, h, data = frame
        shape = (h, w, 3)
        arr = np.frombuffer(data, dtype=np.uint8)
        return arr.reshape(shape)
    
    def poll_frame_pil(self) -> Union["Image.Image", None]:
        """
        Get a frame from the camera. Returns a pillow image.
        Guaranteed to never block, but may return None if no frames were received from camera yet.
        """
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
    result = map(lambda x: CameraInfo(*x), omni_camera.query())
    if only_usable:
        result = filter(CameraInfo.can_open, result)
    return list(result)

