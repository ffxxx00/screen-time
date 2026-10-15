"""Rebuild the original clock icon assets (requires Pillow)."""
from pathlib import Path
import math
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[1] / 'assets'
size = 1024
im = Image.new('RGBA', (size, size))
mask = Image.new('L', (size, size))
draw = ImageDraw.Draw(mask)
draw.rounded_rectangle((16,16,1008,1008),radius=228,fill=255)
# Quiet blue-violet gradient under a simple high-contrast clock face.
gradient = Image.new('RGBA',(size,size))
gd=ImageDraw.Draw(gradient)
for y in range(size):
    t=y/(size-1)
    gd.line((0,y,size,y),fill=(round(92-47*t),round(104-41*t),round(239-43*t),255))
im.paste(gradient,(0,0),mask)
d=ImageDraw.Draw(im)
d.ellipse((193,205,831,843),fill=(20,30,100,45))
d.ellipse((186,186,838,838),fill=(255,255,255,255))
for hour in range(12):
    a=hour*math.pi/6
    radius=269
    x=512+math.sin(a)*radius; y=512-math.cos(a)*radius
    r=10 if hour%3 else 13
    d.ellipse((x-r,y-r,x+r,y+r),fill=(145,153,188,255))
def hand(end,width,color):
    d.line((512,512,*end),fill=color,width=width)
    r=width/2
    for x,y in [(512,512),end]:d.ellipse((x-r,y-r,x+r,y+r),fill=color)
hand((395,403),40,(43,51,82,255))
hand((663,361),32,(43,51,82,255))
hand((592,658),12,(27,163,186,255))
d.ellipse((487,487,537,537),fill=(43,51,82,255))
im=im.resize((256,256),Image.Resampling.LANCZOS)
im.save(root/'app-icon.png')
im.save(root/'app.ico',sizes=[(16,16),(20,20),(24,24),(32,32),(40,40),(48,48),(64,64),(128,128),(256,256)])
(root/'app-icon.rgba').write_bytes(im.resize((32,32),Image.Resampling.LANCZOS).tobytes())
