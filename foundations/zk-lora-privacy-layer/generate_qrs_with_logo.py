import os
import qrcode
from PIL import Image, ImageDraw

def generate_custom_qr(url, filename, logo_path="logo.png"):
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
    
    # 2. Extract the gold lock from the main logo
    if os.path.exists(logo_path):
        logo_img = Image.open(logo_path)
        # Crop the gold lock region (approx center of the 1024x1024 logo)
        # The lock is centered horizontally and slightly higher vertically
        lock_box = (220, 180, 804, 764) # 584x584 square
        lock_logo = logo_img.crop(lock_box)
        
        # Center box size in the QR code (approx 24% of total width)
        box_size = int(width * 0.24)
        lock_logo = lock_logo.resize((box_size, box_size), Image.Resampling.LANCZOS)
        
        x1 = (width - box_size) // 2
        y1 = (height - box_size) // 2
        x2 = x1 + box_size
        y2 = y1 + box_size
        
        draw = ImageDraw.Draw(qr_img)
        # Draw a black border to isolate the logo from the QR code modules
        draw.rectangle([x1 - 3, y1 - 3, x2 + 3, y2 + 3], fill="black")
        
        # Paste the gold lock logo
        qr_img.paste(lock_logo, (x1, y1))
    else:
        print(f"Warning: {logo_path} not found. Drawing fallback gold box.")
        box_size = int(width * 0.24)
        x1 = (width - box_size) // 2
        y1 = (height - box_size) // 2
        x2 = x1 + box_size
        y2 = y1 + box_size
        draw = ImageDraw.Draw(qr_img)
        draw.rectangle([x1 - 2, y1 - 2, x2 + 2, y2 + 2], fill="black")
        draw.rectangle([x1, y1, x2, y2], fill=(243, 179, 0))
        
    # Save the custom QR code
    qr_img.save(filename)
    print(f"Saved custom QR code with gold lock: {filename}")

if __name__ == "__main__":
    generate_custom_qr("https://github.com/DannyB-bit/zk-lora-privacy-layer", "qr_main.png")
    generate_custom_qr("https://github.com/DannyB-bit/zk-lora-milestone-1", "qr_m1.png")
    generate_custom_qr("https://github.com/DannyB-bit/zk-lora-milestone-2", "qr_m2.png")
    generate_custom_qr("https://github.com/DannyB-bit/zk-lora-milestone-3", "qr_m3.png")
