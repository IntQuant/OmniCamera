"""
Select a format and capture an image from camera.
Note: example requires pillow to be installed.
"""
import camerata
import time
cam = camerata.Camera(camerata.query()[0]) # Create a camera
print(*cam.get_format_options().prefer_fps_range(25, 60), sep="\n")
fmt = cam.get_format_options().prefer_fps_range(25, 60).prefer_sides_ratio(800/504).prefer_frame_format(camerata.FrameFormat.MJPEG).resolve() # Select a format from a list of all possible ones
cam.open(fmt) # And use it
while cam.poll_frame_pil() is None: # Note that .poll_frame_* functions never blocks
    time.sleep(0.1) # Wait until we get at least one frame from the camera
#time.sleep(1) # You might want to wait a bit longer while camera is calibrating
img = cam.poll_frame_pil()
img.save("img.png")
