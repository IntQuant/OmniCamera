"""
[Experimental]
Using camera controls.
Note: example requires pillow to be installed.
"""
import omni_camera
import time
cam = omni_camera.Camera(omni_camera.query()[0]) # Open a camera
while cam.poll_frame_pil() is None: 
    time.sleep(0.1) # Wait until we get at least one frame from the camera
time.sleep(1)

img = cam.poll_frame_pil()
img.save("img_baseline.png")

controls = cam.get_controls() # Get a dictionary of all supported controls
print(controls.keys())

control = controls.get("Brightness", None) # Note that not all cameras support all controls
if control is not None:
    control.set_fraction(0.2) # Can use values in 0..1 range with set_fraction
    time.sleep(3)
    img = cam.poll_frame_pil()
    img.save("img_bri_0.2.png")
    control.set_fraction(0.7) 
    time.sleep(3)
    img = cam.poll_frame_pil()
    img.save("img_bri_0.7.png")
    control.set_value(None) # Reset


# White balance doesn't seem to work for me
control = controls.get("WhiteBalance", None)
if control is not None:
    print(control.value_range)
    assert 4000 in control.value_range # set_value only accepts values in this range (again, different cameras have different ranges)
    control.set_value(4000) # Can also set raw values
    time.sleep(3)
    img = cam.poll_frame_pil()
    img.save("img_bal_3000.png")
    control.set_value(None) # Reset