"""
Query a list of available cameras
"""
import omni_camera
print(*omni_camera.query(), sep='\n')
