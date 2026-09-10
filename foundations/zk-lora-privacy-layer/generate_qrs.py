import os
import qrcode
from PIL import Image, ImageDraw, ImageFont

def generate_custom_qr(url, filename, label_text="ZK-LORA"):
    # 1. Generate QR code with High Error Correction (H)
    qr = qrcode.QRCode(
        version=4,
        error_correction=qrcode.constants.ERROR_CORRECT_H,
        box_size=10,
        border=4,
    )
    qr.add_data(url)
    qr.make(fit=True)
    
    # Create QR image (black and white)
    qr_img = qr.make_image(fill_color="black", back_color="white").convert("RGB")
    width, height = qr_img.size
    
    # 2. Draw a gold logo box in the center
    # Center box size (22% of total width)
    box_size = int(width * 0.22)
    x1 = (width - box_size) // 2
    y1 = (height - box_size) // 2
    x2 = x1 + box_size
    y2 = y1 + box_size
    
    draw = ImageDraw.Draw(qr_img)
    
    # Draw outer black border for the center box
    draw.rectangle([x1 - 2, y1 - 2, x2 + 2, y2 + 2], fill="black")
    # Draw the gold box
    gold_color = (243, 179, 0) # Zcash Gold #F3B300
    draw.rectangle([x1, y1, x2, y2], fill=gold_color)
    
    # 3. Draw "ZK" text inside the gold box
    # Find a good bold font
    font_paths = [
        "C:\\Windows\\Fonts\\ariblk.ttf",   # Arial Black
        "C:\\Windows\\Fonts\\arialbd.ttf",   # Arial Bold
        "arial.ttf"
    ]
    font = None
    for path in font_paths:
        if os.path.exists(path):
            try:
                font = ImageFont.truetype(path, int(box_size * 0.5))
                break
            except:
                pass
    if not font:
        font = ImageFont.load_default()
        
    # Center text "ZK"
    text = "ZK"
    try:
        bbox = draw.textbbox((0, 0), text, font=font)
        text_w = bbox[2] - bbox[0]
        text_h = bbox[3] - bbox[1]
    except AttributeError:
        text_w, text_h = draw.textsize(text, font=font)
        
    tx = x1 + (box_size - text_w) // 2
    ty = y1 + (box_size - text_h) // 2 - 2 # slight offset for visual balance
    
    # Draw text in black for high contrast on gold
    draw.text((tx, ty), text, fill="black", font=font)
    
    # Save the custom QR code
    qr_img.save(filename)
    print(f"Saved custom QR code: {filename}")

if __name__ == "__main__":
    generate_custom_qr("https://github.com/DannyB-bit/zk-lora-privacy-layer", "qr_main.png")
    generate_custom_qr("https://github.com/DannyB-bit/zk-lora-milestone-1", "qr_m1.png")
