import os
from PIL import Image, ImageDraw, ImageFont, ImageFilter

def generate_metallic_gold_quote():
    # Exact text and line breaks
    text_lines = [
        '"The impossible is just code waiting to be written, physics waiting to be rewritten,',
        'math a work in progress, and truth waiting to be discovered."'
    ]
    
    width = 1200
    height = 160
    
    # Render at 3x scale for ultra-sharp photorealistic anti-aliasing
    scale = 3
    w_s, h_s = width * scale, height * scale
    
    mask = Image.new("L", (w_s, h_s), 0)
    draw = ImageDraw.Draw(mask)
    
    # Font selection - using thick fonts for a solid 3D metallic presence
    font_paths = [
        "C:\\Windows\\Fonts\\ariblk.ttf",   # Arial Black (Thickest, best for 3D)
        "C:\\Windows\\Fonts\\arialbd.ttf",   # Arial Bold
        "arial.ttf"
    ]
    font = None
    for path in font_paths:
        if os.path.exists(path):
            try:
                # Arial Black needs a slightly smaller size than Arial Bold to fit
                size = 23 * scale if "ariblk" in path.lower() else 25 * scale
                font = ImageFont.truetype(path, size)
                break
            except:
                pass
    if not font:
        font = ImageFont.load_default()
        
    # Draw centered text on mask
    y_offset = 26 * scale
    for line in text_lines:
        try:
            bbox = draw.textbbox((0, 0), line, font=font)
            line_w = bbox[2] - bbox[0]
            line_h = bbox[3] - bbox[1]
        except AttributeError:
            line_w, line_h = draw.textsize(line, font=font)
            
        x = (w_s - line_w) // 2
        draw.text((x, y_offset), line, fill=255, font=font)
        y_offset += line_h + 16 * scale

    # Convert mask to float for processing
    mask_im = mask.copy()
    
    # 2. Create 3D Bevel effect (Multiple offsets for smooth rounded bevel)
    # We blend multiple shifted versions to create a smooth 3D bevel contour
    bevel_hl = Image.new("L", (w_s, h_s), 0)
    bevel_sd = Image.new("L", (w_s, h_s), 0)
    
    # Shift top-left for highlight, bottom-right for shadow
    shift_h = 3
    shift_s = 4
    
    # Create shifted masks
    hl_mask = Image.new("L", (w_s, h_s), 0)
    sd_mask = Image.new("L", (w_s, h_s), 0)
    hl_mask.paste(mask_im, (-shift_h, -shift_h))
    sd_mask.paste(mask_im, (shift_s, shift_s))
    
    # Extract edges
    # Highlights = mask AND NOT hl_mask
    # Shadows = mask AND NOT sd_mask
    
    # 3. Create Metallic Gold Reflection Gradient (100% gold, no white/silver)
    gradient = Image.new("RGB", (w_s, h_s))
    stops = [
        (0.0, (120, 80, 15)),    # Deep Bronze Gold
        (0.2, (212, 175, 55)),   # Metallic Gold
        (0.35, (255, 215, 0)),   # Pure Gold
        (0.5, (255, 235, 140)),  # Warm Glow Gold (maximum brightness stop, no white)
        (0.65, (255, 215, 0)),   # Pure Gold
        (0.8, (212, 175, 55)),   # Metallic Gold
        (1.0, (120, 80, 15))     # Deep Bronze Gold
    ]
    
    for y in range(h_s):
        t = y / float(h_s)
        c1, c2 = stops[0][1], stops[-1][1]
        t1, t2 = 0.0, 1.0
        for i in range(len(stops)-1):
            if stops[i][0] <= t <= stops[i+1][0]:
                t1, c1 = stops[i]
                t2, c2 = stops[i+1]
                break
        factor = (t - t1) / (t2 - t1) if t2 > t1 else 0.0
        r = int(c1[0] + (c2[0] - c1[0]) * factor)
        g = int(c1[1] + (c2[1] - c1[1]) * factor)
        b = int(c1[2] + (c2[2] - c1[2]) * factor)
        
        for x in range(w_s):
            gradient.putpixel((x, y), (r, g, b))

    # 4. Render the 3D text
    text_gold = Image.new("RGB", (w_s, h_s), (0, 0, 0))
    text_gold.paste(gradient, (0, 0), mask=mask_im)
    
    # Add inner shadow/depth by drawing a contracted dark gold overlay
    contracted_mask = mask_im.filter(ImageFilter.MinFilter(3))
    inner_gold_color = (255, 223, 100) # Bright gold core
    
    # Layering bevel highlights and shadows
    hl_color = (255, 240, 170) # Warm Glow Gold Highlight
    sd_color = (70, 45, 10)     # Dark Bronze Shadow
    
    # Create 3D extrusion/drop shadow effect on the background
    extrusion = Image.new("RGB", (w_s, h_s), (0, 0, 0))
    draw_ex = ImageDraw.Draw(extrusion)
    
    # Draw multiple offset layers of dark shadow to create a 3D extrusion look
    for d in range(1, 6):
        y_offset = 26 * scale
        for line in text_lines:
            try:
                bbox = draw_ex.textbbox((0, 0), line, font=font)
                line_w = bbox[2] - bbox[0]
                line_h = bbox[3] - bbox[1]
            except AttributeError:
                line_w, line_h = draw_ex.textsize(line, font=font)
            x = (w_s - line_w) // 2
            draw_ex.text((x + d, y_offset + d), line, fill=sd_color, font=font)
            y_offset += line_h + 16 * scale
            
    # Combine extrusion, base gold, and inner highlights
    final_3d = Image.new("RGB", (w_s, h_s), (0, 0, 0))
    # Paste extrusion first (outside the mask)
    final_3d.paste(extrusion, (0, 0))
    # Paste the shiny gold text on top
    final_3d.paste(text_gold, (0, 0), mask=mask_im)
    
    # Draw a thin warm gold outline/highlight on the top-left edges
    hl_img = Image.new("RGB", (w_s, h_s), (0, 0, 0))
    draw_hl = ImageDraw.Draw(hl_img)
    y_offset = 26 * scale
    for line in text_lines:
        try:
            bbox = draw_hl.textbbox((0, 0), line, font=font)
            line_w = bbox[2] - bbox[0]
            line_h = bbox[3] - bbox[1]
        except AttributeError:
            line_w, line_h = draw_hl.textsize(line, font=font)
        x = (w_s - line_w) // 2
        draw_hl.text((x - 1, y_offset - 1), line, fill=hl_color, font=font)
        y_offset += line_h + 16 * scale
        
    # Paste the highlight on top-left edges
    final_3d.paste(hl_img, (0, 0), mask=mask_im)
    
    # 5. Scale down with high-quality Lanczos resampling for perfect anti-aliasing
    final_3d_rescaled = final_3d.resize((width, height), Image.Resampling.LANCZOS)
    
    # Save image
    final_3d_rescaled.save("gold_quote.png")
    print("Generated 3D metallic gold_quote.png successfully!")

if __name__ == "__main__":
    generate_metallic_gold_quote()
