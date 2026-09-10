import os
import sys
import math
from fpdf import FPDF
from fpdf.enums import TableCellFillMode

# Ensure UTF-8 output encoding on Windows
import io
if sys.platform == "win32":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

class PDF(FPDF):
    def header(self):
        # Small running header from page 2 onwards
        if self.page_no() > 1:
            self.set_draw_color(28, 54, 115)
            self.set_line_width(0.85)
            self.line(15, 10, self.w - 15, 10)
            
            self.set_font("Helvetica", "B", 9)
            self.set_text_color(35, 35, 35)
            self.cell(0, 10, "ZYMATICA | VOICE LLM WHITEPAPER", align="R")
            self.ln(12)

    def footer(self):
        self.set_y(-15)
        self.set_font("Helvetica", "", 8)
        self.set_text_color(45, 45, 45)
        # Page divider line
        self.set_draw_color(80, 80, 80)
        self.set_line_width(0.85)
        self.line(15, self.y - 2, self.w - 15, self.y - 2)
        
        self.cell(0, 10, "© 2026 Zymatica.space | astronautshe.com | DevsOne | We Are TheAiCollective.art", align="L")
        self.set_x(-30)
        self.cell(0, 10, f"Page {self.page_no()}", align="R")

# --- CUSTOM DRAWING HELPERS ---

def draw_box(pdf, x, y, w, h, text, fill_color, text_color, font_size=8.5, is_bold=False):
    pdf.set_fill_color(*fill_color)
    pdf.set_draw_color(40, 40, 40)
    pdf.set_line_width(0.85)
    pdf.rect(x, y, w, h, style="FD")
    
    pdf.set_text_color(*text_color)
    pdf.set_font("Helvetica", "B" if is_bold else "", font_size)
    
    text_w = pdf.get_string_width(text)
    tx = x + (w - text_w) / 2
    font_h_mm = font_size * 0.3527
    ty = y + (h + font_h_mm * 0.6) / 2
    pdf.text(tx, ty, text)

def draw_arrow(pdf, x1, y1, x2, y2, label=None, label_pos="above"):
    pdf.set_draw_color(40, 40, 40)
    pdf.set_line_width(0.85)
    pdf.line(x1, y1, x2, y2)
    
    angle = math.atan2(y2 - y1, x2 - x1)
    arrow_len = 4.0
    ax1 = x2 - arrow_len * math.cos(angle - math.pi/6)
    ay1 = y2 - arrow_len * math.sin(angle - math.pi/6)
    ax2 = x2 - arrow_len * math.cos(angle + math.pi/6)
    ay2 = y2 - arrow_len * math.sin(angle + math.pi/6)
    
    pdf.set_fill_color(40, 40, 40)
    pdf.polygon([(x2, y2), (ax1, ay1), (ax2, ay2)], style="F")
    
    if label:
        pdf.set_font("Helvetica", "B", 7.0)
        pdf.set_text_color(15, 15, 15)
        
        if abs(x1 - x2) < 0.1:
            lx = x1 + 2.0
            ly = (y1 + y2) / 2 + 1.0
            pdf.text(lx, ly, label)
        else:
            lbl_w = pdf.get_string_width(label)
            lx = (x1 + x2) / 2 - lbl_w / 2
            ly = (y1 + y2) / 2
            if label_pos == "above":
                ly -= 2.0
            elif label_pos == "below":
                ly += 3.5
            pdf.text(lx, ly, label)

def draw_voice_link_diagram(pdf):
    y_start = pdf.get_y()
    
    pdf.set_fill_color(250, 250, 250)
    pdf.set_draw_color(40, 40, 40)
    pdf.set_line_width(1.20)
    pdf.rect(15, y_start, 180, 52, style="FD")
    
    pdf.set_font("Helvetica", "B", 9)
    pdf.set_text_color(0, 0, 0)
    pdf.text(20, y_start + 5, "ZYMATICA VOICE COMMS LINK & LEVEL 9 DEFLATE AUDIO PIPELINE")
    
    draw_box(pdf, x=18, y=y_start + 10, w=36, h=10, text="1. User Mic / Web ASR", fill_color=(235, 240, 250), text_color=(28, 54, 115), is_bold=True, font_size=7.5)
    pdf.set_font("Helvetica", "", 6.5)
    pdf.set_text_color(15, 15, 15)
    pdf.text(19, y_start + 23, "Continuous transcription")
    
    draw_arrow(pdf, 54, y_start + 15, 71.5, y_start + 15, label="HTTPS text", label_pos="above")
    
    draw_box(pdf, x=73, y=y_start + 10, w=36, h=10, text="2. Fast LLM Router", fill_color=(235, 240, 250), text_color=(0, 0, 0), is_bold=True, font_size=7.5)
    pdf.text(74, y_start + 23, "Groq / Nvidia NIM / OpenAI")
    
    draw_arrow(pdf, 109, y_start + 15, 126.5, y_start + 15, label="sentences", label_pos="above")
    
    draw_box(pdf, x=128, y=y_start + 10, w=36, h=10, text="3. Sentence TTS", fill_color=(235, 240, 250), text_color=(0, 0, 0), is_bold=True, font_size=7.5)
    pdf.text(129, y_start + 23, "VibeVoice / Edge-TTS")
    
    pdf.set_draw_color(40, 40, 40)
    pdf.set_line_width(0.85)
    pdf.line(164, y_start + 15, 172, y_start + 15)
    pdf.line(172, y_start + 15, 172, y_start + 35)
    draw_arrow(pdf, 172, y_start + 35, 165.5, y_start + 35)
    
    pdf.set_font("Helvetica", "", 6.5)
    pdf.set_text_color(15, 15, 15)
    pdf.text(174, y_start + 25, "raw WAV")
    
    draw_box(pdf, x=128, y=y_start + 31, w=36, h=8, text="4. Level 9 Deflate", fill_color=(28, 54, 115), text_color=(255, 255, 255), is_bold=True, font_size=7.2)
    
    draw_arrow(pdf, 128, y_start + 35, 110.5, y_start + 35, label="50-75% smaller bytes", label_pos="above")
    
    draw_box(pdf, x=73, y=y_start + 31, w=36, h=8, text="5. Web Decompress", fill_color=(245, 245, 245), text_color=(0, 0, 0), is_bold=True, font_size=7.2)
    
    draw_arrow(pdf, 73, y_start + 35, 55.5, y_start + 35, label="PCM WAV", label_pos="above")
    
    draw_box(pdf, x=18, y=y_start + 31, w=36, h=8, text="6. Buffered Queue", fill_color=(28, 54, 115), text_color=(255, 255, 255), is_bold=True, font_size=7.2)
    
    pdf.set_draw_color(40, 40, 40)
    pdf.set_line_width(0.85)
    pdf.line(18, y_start + 35, 10, y_start + 35)
    pdf.line(10, y_start + 35, 10, y_start + 15)
    draw_arrow(pdf, 10, y_start + 15, 16.5, y_start + 15, label="0ms Player Gap", label_pos="above")
    
    pdf.set_y(y_start + 49)
    pdf.ln(3)

def draw_zrdt_diagram(pdf):
    y_start = pdf.get_y()
    
    pdf.set_fill_color(250, 250, 250)
    pdf.set_draw_color(40, 40, 40)
    pdf.set_line_width(1.20)
    pdf.rect(15, y_start, 180, 52, style="FD")
    
    pdf.set_font("Helvetica", "B", 9)
    pdf.set_text_color(0, 0, 0)
    pdf.text(20, y_start + 5, "ZYMATICA REAL-TIME DIALECTIC TRAINING (ZRDT) CLOSED LOOP")
    
    # 1. Dialogue Simulation
    draw_box(pdf, x=18, y=y_start + 10, w=40, h=10, text="1. Dialogue Simulation", fill_color=(235, 240, 250), text_color=(28, 54, 115), is_bold=True, font_size=7.5)
    pdf.set_font("Helvetica", "", 6.5)
    pdf.set_text_color(15, 15, 15)
    pdf.text(20, y_start + 23, "Girlfriend <--> Boyfriend")
    
    draw_arrow(pdf, 58, y_start + 15, 75.5, y_start + 15, label="Dialogue Turns", label_pos="above")
    
    # 2. Telemetry extraction
    draw_box(pdf, x=77, y=y_start + 10, w=40, h=10, text="2. Telemetry Extract", fill_color=(235, 240, 250), text_color=(0, 0, 0), is_bold=True, font_size=7.5)
    pdf.text(78, y_start + 23, "Latencies, check, MD5")
    
    draw_arrow(pdf, 117, y_start + 15, 134.5, y_start + 15, label="Metrics Payload", label_pos="above")
    
    # 3. Z Agent Observers
    draw_box(pdf, x=136, y=y_start + 10, w=40, h=10, text="3. Z Agent Observers", fill_color=(28, 54, 115), text_color=(255, 255, 255), is_bold=True, font_size=7.5)
    pdf.text(137, y_start + 23, "Z Agent-A & Z Agent-B")
    
    # Flow down to step 4
    pdf.set_draw_color(40, 40, 40)
    pdf.set_line_width(0.85)
    pdf.line(156, y_start + 15, 164, y_start + 15)
    pdf.line(164, y_start + 15, 164, y_start + 35)
    draw_arrow(pdf, 164, y_start + 35, 156.5, y_start + 35)
    
    pdf.set_font("Helvetica", "", 6.5)
    pdf.set_text_color(15, 15, 15)
    pdf.text(166, y_start + 25, "Critiques")
    
    # 4. Prompt Calibration
    draw_box(pdf, x=116, y=y_start + 31, w=40, h=8, text="4. Prompt Calibration", fill_color=(28, 54, 115), text_color=(255, 255, 255), is_bold=True, font_size=7.2)
    
    draw_arrow(pdf, 116, y_start + 35, 93.5, y_start + 35, label="Calibration Prompts", label_pos="above")
    
    # 5. Weight Adaptation
    draw_box(pdf, x=52, y=y_start + 31, w=40, h=8, text="5. Weight Adaptation", fill_color=(220, 240, 225), text_color=(20, 80, 40), is_bold=True, font_size=7.2)
    
    # Arrow back to simulation (horizontal to margin, vertical up, point to step 1)
    pdf.line(52, y_start + 35, 10, y_start + 35)
    pdf.line(10, y_start + 35, 10, y_start + 15)
    draw_arrow(pdf, 10, y_start + 15, 16.5, y_start + 15, label="Self-Correction", label_pos="above")
    
    pdf.set_y(y_start + 49)
    pdf.ln(3)

def main():
    project_dir = os.path.dirname(os.path.abspath(__file__))
    md_path = os.path.join(project_dir, "zymatica_voice_llm_whitepaper.md")
    pdf_path = os.path.join(project_dir, "Zymatica_Voice_LLM_Whitepaper.pdf")
    logo_path = os.path.join(project_dir, "Logo.png")

    if not os.path.exists(md_path):
        print(f"Error: Markdown file not found at {md_path}")
        return

    print("Generating Zymatica Voice LLM Whitepaper PDF...")
    pdf = PDF()
    pdf.set_margins(15, 15, 15)
    pdf.add_page()
    pdf.set_auto_page_break(auto=True, margin=22)

    # 1. Title Page Logo
    if os.path.exists(logo_path):
        pdf.image(logo_path, x=80, y=20, w=50)
        pdf.ln(60)
    else:
        pdf.ln(15)

    # 2. Main Title
    pdf.set_font("Helvetica", "B", 18)
    pdf.set_text_color(28, 54, 115)
    pdf.multi_cell(0, 10, "ZYMATICA VOICE LLM WHITEPAPER", align="C", new_x="LMARGIN", new_y="NEXT")
    
    pdf.set_font("Helvetica", "B", 11)
    pdf.set_text_color(35, 35, 35)
    pdf.cell(0, 8, "A Low-Latency Dialectic Speech Agent with Real-Time Reinforcement", align="C", new_x="LMARGIN", new_y="NEXT")
    pdf.cell(0, 6, "Version 1.0 | Technical Report", align="C", new_x="LMARGIN", new_y="NEXT")
    pdf.ln(10)

    with open(md_path, "r", encoding="utf-8") as f:
        lines = f.readlines()

    replacements = {
        "’": "'", "‘": "'", "“": '"', "”": '"', "–": "-", "—": "-", "…": "...",
        "\u2013": "-", "\u2014": "-", "\u2019": "'", "\u2018": "'", "\u201c": '"', "\u201d": '"',
        "•": "-", "✔": "x", "™": "(TM)", "®": "(R)", "©": "(C)", "🛸": "", "🧠": "", "🛡️": "",
        "🗜️": "", "⚖️": "", "🎨": "", "🔮": "", "❤️": "", "⚠️": "[WARNING]", "👤": "User", "🤖": "Bot"
    }

    def clean(text):
        for k, v in replacements.items():
            text = text.replace(k, v)
        return text.encode('latin-1', 'ignore').decode('latin-1')

    def render_table(pdf, table_rows):
        if not table_rows:
            return
        pdf.set_fill_color(255, 255, 255)
        pdf.set_text_color(15, 15, 15)
        pdf.set_font("Helvetica", size=8.5)
        pdf.set_draw_color(100, 100, 100)
        pdf.set_line_width(0.4)
        
        cleaned_rows = []
        for row in table_rows:
            cleaned_row = []
            for cell in row:
                cleaned_cell = cell.replace("`", "").replace("**", "")
                cleaned_row.append(cleaned_cell)
            cleaned_rows.append(cleaned_row)
            
        col_count = len(cleaned_rows[0]) if cleaned_rows else 4
        if col_count == 6:
            widths = (40, 28, 28, 28, 28, 28)
        elif col_count == 5:
            widths = (48, 33, 33, 33, 33)
        else:
            widths = (38, 26, 32, 84)
        with pdf.table(
            markdown=False,
            cell_fill_mode=TableCellFillMode.EVEN_ROWS,
            cell_fill_color=(242, 245, 249),
            col_widths=widths,
            align="LEFT",
            width=pdf.w - pdf.l_margin - pdf.r_margin
        ) as t:
            for row in cleaned_rows:
                t.row(row)
        pdf.ln(3)

    def print_bullet(pdf, text, bold_phrase=None):
        if pdf.get_y() > pdf.h - 32:
            pdf.add_page()
        original_margin = pdf.l_margin
        bullet_indent = 8
        text_indent = 16
        
        pdf.set_x(original_margin + bullet_indent)
        pdf.set_font("Helvetica", "", 10.5)
        pdf.cell(4, 5, chr(149), align='C') 
        current_y = pdf.get_y()

        pdf.set_left_margin(original_margin + text_indent)
        pdf.set_y(current_y)
        pdf.set_x(original_margin + text_indent)
        
        if bold_phrase:
            full_text = f"**{bold_phrase.strip()}** {text.strip()}"
            pdf.multi_cell(0, 5, clean(full_text), markdown=True, new_x="LMARGIN", new_y="NEXT")
        else:
            pdf.multi_cell(0, 5, clean(text.strip()), markdown=True, new_x="LMARGIN", new_y="NEXT")
            
        pdf.set_left_margin(original_margin)
        pdf.ln(1.5)

    in_code_block = False
    code_text = []
    in_table = False
    table_rows = []

    for line in lines:
        line_stripped = line.strip()
        
        if in_code_block:
            if line_stripped.startswith("```"):
                block_content = "\n".join(code_text)
                
                # Check diagrams
                if "templates/phone_call.html" in block_content or "zlib Compressing" in block_content:
                    if pdf.get_y() + 55 > pdf.h - 22:
                        pdf.add_page()
                    draw_voice_link_diagram(pdf)
                elif "ZRDT Evaluation Loop" in block_content:
                    if pdf.get_y() + 55 > pdf.h - 22:
                        pdf.add_page()
                    draw_zrdt_diagram(pdf)
                else:
                    est_h = len(code_text) * 4.5 + 10
                    if pdf.get_y() + est_h > pdf.h - 22:
                        pdf.add_page()
                    pdf.set_font("Courier", size=8.5)
                    pdf.set_text_color(60, 60, 60)
                    pdf.set_fill_color(245, 245, 245)
                    pdf.multi_cell(0, 4.5, clean(block_content), fill=True, new_x="LMARGIN", new_y="NEXT")
                    pdf.ln(3)
                    
                code_text = []
                in_code_block = False
            else:
                code_text.append(line.rstrip('\n'))
            continue

        if line_stripped.startswith("|"):
            if all(c in " |:-" for c in line_stripped):
                in_table = True
                continue
            cells = [cell.strip() for cell in line_stripped.split("|")[1:-1]]
            table_rows.append(cells)
            in_table = True
            continue
        
        if in_table:
            render_table(pdf, table_rows)
            table_rows = []
            in_table = False

        if line_stripped.startswith("```"):
            in_code_block = True
            continue

        if line_stripped.startswith("# ") or line_stripped.startswith("!["):
            continue

        if not line_stripped:
            pdf.ln(3) 
            continue

        line_cleaned = clean(line_stripped)
        
        if line_stripped.startswith("## "):
            if pdf.get_y() + 25 > pdf.h - 22:
                pdf.add_page()
            pdf.ln(5)
            pdf.set_fill_color(28, 54, 115)
            pdf.set_text_color(255, 255, 255)
            pdf.set_font("Helvetica", "B", 11.5)
            text = line_stripped.replace("## ", "").strip()
            pdf.multi_cell(0, 7.5, clean(text), fill=True, align='L', new_x="LMARGIN", new_y="NEXT")
            pdf.ln(2.5)
            pdf.set_text_color(15, 15, 15)
            pdf.set_font("Helvetica", size=10.5)

        elif line_stripped.startswith("### "):
            if pdf.get_y() + 20 > pdf.h - 22:
                pdf.add_page()
            pdf.ln(2.5)
            pdf.set_font("Helvetica", "B", 10.5)
            pdf.set_text_color(28, 54, 115)
            text = line_stripped.replace("### ", "").strip()
            pdf.multi_cell(0, 5.5, clean(text), align='L', markdown=True, new_x="LMARGIN", new_y="NEXT")
            pdf.set_text_color(15, 15, 15)
            pdf.set_font("Helvetica", size=10.5)

        elif line_stripped == "---":
            pdf.ln(3)
            pdf.set_draw_color(80, 80, 80)
            pdf.set_line_width(0.85)
            pdf.line(pdf.get_x(), pdf.get_y(), pdf.w - pdf.r_margin, pdf.get_y())
            pdf.ln(3)

        elif line_stripped.startswith("- **") or line_stripped.startswith("* **"):
            prefix = "- " if line_stripped.startswith("-") else "* "
            parts = line_stripped[len(prefix):].split("**")
            if len(parts) >= 3:
                header = parts[1]
                rest = "".join(parts[2:])
                print_bullet(pdf, rest, bold_phrase=header)
            else:
                print_bullet(pdf, line_stripped[len(prefix):])

        elif line_stripped.startswith("- ") or line_stripped.startswith("* "):
            prefix = "- " if line_stripped.startswith("-") else "* "
            print_bullet(pdf, line_stripped[len(prefix):])

        else:
            num_lines = math.ceil(len(line_cleaned) / 95)
            est_h = num_lines * 5.5 + 2
            if pdf.get_y() + est_h > pdf.h - 22:
                pdf.add_page()
                
            pdf.set_font("Helvetica", size=10.5)
            pdf.set_text_color(15, 15, 15)
            pdf.multi_cell(0, 5.5, line_cleaned, markdown=True, new_x="LMARGIN", new_y="NEXT")
            pdf.ln(1.5)

    if in_table and table_rows:
        render_table(pdf, table_rows)

    if in_code_block and code_text:
        block_content = "\n".join(code_text)
        pdf.set_font("Courier", size=8.5)
        pdf.set_text_color(60, 60, 60)
        pdf.set_fill_color(245, 245, 245)
        pdf.multi_cell(0, 4.5, clean(block_content), fill=True, new_x="LMARGIN", new_y="NEXT")
            
    # Render the sign-off block
    pdf.ln(3)
    pdf.set_draw_color(80, 80, 80)
    pdf.set_line_width(0.85)
    pdf.line(15, pdf.get_y(), pdf.w - 15, pdf.get_y())
    pdf.ln(5)
    
    if pdf.get_y() + 55 > pdf.h - 22:
        pdf.add_page()
        
    pdf.ln(2)
    pdf.set_font("Helvetica", "I", 10.5)
    pdf.set_text_color(15, 15, 15)
    pdf.multi_cell(0, 5.5, clean('“The impossible is just code waiting to be written, physics waiting to be rewritten, math a work in progress, and voice training a loop waiting to close.”'), align="C", new_x="LMARGIN", new_y="NEXT")
    pdf.ln(2.5)
    
    pdf.set_font("Helvetica", "", 10)
    pdf.multi_cell(0, 5.5, clean("This is not voice playback. This is real-time reinforcement learning and dialectic alignment —\nthe engineering standard for verifiable agent communication."), align="C", new_x="LMARGIN", new_y="NEXT")
    pdf.ln(4.5)
    
    y_box_start = pdf.get_y()
    box_h = 32
    
    pdf.set_fill_color(245, 248, 255)
    pdf.set_draw_color(28, 54, 115)
    pdf.set_line_width(0.6)
    pdf.rect(15, y_box_start, 180, box_h, style="FD")
    
    pdf.set_y(y_box_start + 2.5)
    pdf.set_x(18)
    pdf.set_font("Helvetica", "B", 9)
    pdf.set_text_color(28, 54, 115)
    pdf.cell(0, 5, "ZYMATICA VOICE LLM SYSTEM AUDIT SIGN OFF:", new_x="LMARGIN", new_y="NEXT")
    
    pdf.set_x(18)
    pdf.set_font("Helvetica", "", 8.5)
    pdf.set_text_color(15, 15, 15)
    pdf.multi_cell(174, 4.5, clean("Framework Core: zymatica.space • Systems Integration: astronautshe.com • Agent Alignment: DevsOne • Brand Publisher:\nTheAiCollective.art"), new_x="LMARGIN", new_y="NEXT")
    pdf.ln(1)
    
    pdf.set_x(18)
    pdf.set_font("Helvetica", "B", 8)
    pdf.set_text_color(45, 45, 45)
    pdf.cell(0, 4, clean("© 2026 All Rights Reserved Zymatica.space"), new_x="LMARGIN", new_y="NEXT")
    pdf.set_x(18)
    pdf.cell(0, 4, clean("Zymatica.space • astronautshe.com • DevsOne"), new_x="LMARGIN", new_y="NEXT")
    pdf.set_x(18)
    pdf.cell(0, 4, clean("We Are TheAiCollective.art"), new_x="LMARGIN", new_y="NEXT")
    
    # Output file
    try:
        pdf.output(pdf_path)
        print(f"Successfully generated PDF voice whitepaper at: {pdf_path}")
    except Exception as ex:
        print(f"Error outputting PDF: {ex}")

if __name__ == "__main__":
    main()
