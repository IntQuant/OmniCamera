# Camerata
A library for querying and capturing from cameras, based on [nokhwa](https://github.com/l1npengtul/nokhwa) crate.

# Examples
Query available cameras:
```
print(*camerata.query(), sep='\n')
```
Example output:
```
CameraInfo(index=2, name='UVC Camera (046d:0809)', description='Video4Linux Device @ /dev/video2', misc='')
CameraInfo(index=0, name='USB2.0 VGA UVC WebCam: USB2.0 V', description='Video4Linux Device @ /dev/video0', misc='')
```


See examples/ for more

