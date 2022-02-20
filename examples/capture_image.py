"""
Capture an image from camera.
Note: example requires pillow to be installed.
"""
import camerata
import time
cam = camerata.Camera(camerata.query()[0]) # Open a camera
while cam.poll_frame_pil() is None: # Note that .poll_frame_* functions never blocks
    time.sleep(0.1) # Wait until we get at least one frame from the camera
#time.sleep(1) # You might want to wait a bit longer while camera is calibrating
img = cam.poll_frame_pil()
img.save("img.png")
