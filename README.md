# OmniCamera
A library for querying and capturing from cameras, based on [nokhwa](https://github.com/l1npengtul/nokhwa) crate.

# Examples
Query available cameras:
```python
print(*omni_camera.query(), sep='\n')
```
Example output:
```
CameraInfo(index=2, name='UVC Camera (046d:0809)', description='Video4Linux Device @ /dev/video2', misc='')
CameraInfo(index=0, name='USB2.0 VGA UVC WebCam: USB2.0 V', description='Video4Linux Device @ /dev/video0', misc='')
```

Save an image (note: requires pillow to be installed):
```python
import omni_camera
import time
cam = omni_camera.Camera(camerata.query()[0]) # Open a camera
while cam.poll_frame_pil() is None: # Note that .poll_frame_* functions never blocks
    time.sleep(0.1) # Wait until we get at least one frame from the camera
#time.sleep(1) # You might want to wait a bit longer while camera is calibrating
img = cam.poll_frame_pil()
img.save("img.png")
```

See examples/ for more

