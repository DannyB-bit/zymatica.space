# -*- coding: utf-8 -*-
import os
import re
import shutil
from reportlab.lib.pagesizes import letter
from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, Table, TableStyle, Image, PageBreak, KeepTogether
from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
from reportlab.lib import colors
from reportlab.pdfgen import canvas

class NumberedCanvas(canvas.Canvas):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._saved_page_states = []

    def showPage(self):
        self._saved_page_states.append(dict(self.__dict__))
        self._startPage()

    def save(self):
        num_pages = len(self._saved_page_states)
        for state in self._saved_page_states:
            self.__dict__.update(state)
            self.draw_page_decorations(num_pages)
            super().showPage()
        super().save()

    def draw_page_decorations(self, page_count):
        self.saveState()
        # Page 1: Full-page Language U Logo
        if self._pageNumber == 1:
            logo_path = "language_u_logo.jpg" if os.path.exists("language_u_logo.jpg") else os.path.join(os.path.dirname(os.path.abspath(__file__)), "language_u_logo.jpg")
            if os.path.exists(logo_path):
                self.drawImage(logo_path, 0, 0, width=letter[0], height=letter[1])
        # Last Page: Full-page AI Collective Logo with custom sign-off
        elif self._pageNumber == page_count:
            logo_path = "Logo.jpg" if os.path.exists("Logo.jpg") else os.path.join(os.path.dirname(os.path.abspath(__file__)), "Logo.jpg")
            if os.path.exists(logo_path):
                self.drawImage(logo_path, 0, 0, width=letter[0], height=letter[1])
            
            # Semi-transparent overlay at the bottom for the sign-off
            self.setFillColor(colors.HexColor("#0D1527"))
            self.rect(0, 0, letter[0], 85, fill=True, stroke=False)
            
            self.setFont("Helvetica-Bold", 10)
            self.setFillColor(colors.white)
            self.drawCentredString(letter[0]/2.0, 48, "zymatica.space  |  astronautshe.com  |  Devs One")
            self.setFont("Helvetica-Bold", 8)
            self.setFillColor(colors.HexColor("#63B3ED"))
            self.drawCentredString(letter[0]/2.0, 28, "We Are TheAiCollective.art  Zymatica Covenant License 2.0 (zymatica.space) 2026©")
        # Middle Pages (Running Headers & Footers)
        else:
            # Running Header
            self.setFont("Helvetica-Bold", 8)
            self.setFillColor(colors.HexColor("#0F2D59"))
            self.drawString(54, 755, "LANGUAGE-U SYSTEM TELEMETRY & OPTIMIZATION")
            
            self.setFont("Helvetica", 8)
            self.setFillColor(colors.HexColor("#718096"))
            self.drawRightString(558, 755, "WASM MICROSECOND PERFORMANCE WHITE PAPER")
            
            # Header line
            self.setStrokeColor(colors.HexColor("#CBD5E0"))
            self.setLineWidth(0.75)
            self.line(54, 747, 558, 747)
            
            # Running Footer
            self.setStrokeColor(colors.HexColor("#CBD5E0"))
            self.setLineWidth(0.75)
            self.line(54, 55, 558, 55)
            
            self.setFont("Helvetica", 8)
            self.setFillColor(colors.HexColor("#718096"))
            self.drawString(54, 42, "zymatica.space | astronautshe.com | Devs One | We Are TheAiCollective.art")
            
            page_text = f"Page {self._pageNumber} of {page_count}"
            self.drawRightString(558, 42, page_text)
            
        self.restoreState()

def clean_md_text(text):
    # Escape HTML special chars except tags we generate
    text = text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
    
    # Restore specific tags
    text = text.replace(r"$$\text{Latency}_{\text{WebGPU}} = t_{\text{buffer\_copy}} + t_{\text{command\_compile}} + t_{\text{queue\_dispatch}} + t_{\text{mapAsync}} \approx 0.115 \text{ ms}$$", 
                        '<i>Latency<sub>WebGPU</sub> = t<sub>buffer_copy</sub> + t<sub>command_compile</sub> + t<sub>queue_dispatch</sub> + t<sub>mapAsync</sub> &asymp; 0.115 ms</i>')
    text = text.replace(r"$$\text{Latency}_{\text{WebGPU}} = t_{\text{buffer\_copy}} + t_{\text{command\_compile}} + t_{\text{queue\_dispatch}} + t_{\text{mapAsync}} \approx 0.115 \text{ ms}$$", "")
    text = text.replace(r"$$", "")
    text = text.replace(r"$\text{Latency}_{\text{WebGPU}}$", "<i>Latency<sub>WebGPU</sub></i>")
    text = text.replace(r"$t_{\text{buffer\_copy}}$", "<i>t<sub>buffer_copy</sub></i>")
    text = text.replace(r"$t_{\text{command\_compile}}$", "<i>t<sub>command_compile</sub></i>")
    text = text.replace(r"$t_{\text{queue\_dispatch}}$", "<i>t<sub>queue_dispatch</sub></i>")
    text = text.replace(r"$t_{\text{mapAsync}}$", "<i>t<sub>mapAsync</sub></i>")
    text = text.replace(r"$t_{\text{mapAsync}} \approx 0.115 \text{ ms}$", "<i>t<sub>mapAsync</sub> &asymp; 0.115 ms</i>")
    text = text.replace(r"$\approx 0.115 \text{ ms}$", "<i>&asymp; 0.115 ms</i>")
    text = text.replace(r"$\approx 0.12 \text{ ms}$", "<i>&asymp; 0.12 ms</i>")
    text = text.replace(r"$\text{META:num\_concepts:payload\_hash:compressed\_len}$", "<b>META:num_concepts:payload_hash:compressed_len</b>")
    text = text.replace(r"$\text{trans\_rc}$", "<i>trans_rc</i>")
    text = text.replace(r"$\text{trans\_rf}$", "<i>trans_rf</i>")
    text = text.replace(r"$\text{trans\_ra}$", "<i>trans_ra</i>")
    
    # Convert bold markdown **text** to <b>text</b>
    text = re.sub(r'\*\*(.*?)\*\*', r'<b>\1</b>', text)
    text = re.sub(r'__(.*?)__', r'<b>\1</b>', text)
    
    # Convert italic markdown *text* to <i>text</i>
    text = re.sub(r'\*(.*?)\*', r'<i>\1</i>', text)
    
    # Convert `code` to <font name="Courier"><b>code</b></font>
    text = re.sub(r'`(.*?)`', r'<b><font name="Courier" color="#0F2D59">\1</font></b>', text)
    
    # Convert links [text](url) to <a href="url">text</a>
    text = re.sub(r'\[(.*?)\]\((.*?)\)', r'<a href="\2"><font color="#1D4ED8"><b>\1</b></font></a>', text)
    
    return text

def parse_markdown(md_path):
    with open(md_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    lines = content.split('\n')
    elements = []
    
    in_code_block = False
    code_lines = []
    
    in_table = False
    table_rows = []
    
    i = 0
    while i < len(lines):
        line = lines[i]
        
        # Code block check
        if line.strip().startswith('```'):
            if in_code_block:
                in_code_block = False
                elements.append({
                    'type': 'code',
                    'content': '\n'.join(code_lines)
                })
                code_lines = []
            else:
                in_code_block = True
            i += 1
            continue
            
        if in_code_block:
            code_lines.append(line)
            i += 1
            continue
            
        # Table check
        if line.strip().startswith('|') and not in_table:
            # Check if next line is table separator
            if i + 1 < len(lines) and lines[i+1].strip().startswith('|') and '-' in lines[i+1]:
                in_table = True
                table_rows = []
                headers = [cell.strip() for cell in line.split('|')[1:-1]]
                table_rows.append(headers)
                i += 2
                continue
                
        if in_table:
            if line.strip().startswith('|'):
                cells = [cell.strip() for cell in line.split('|')[1:-1]]
                table_rows.append(cells)
                i += 1
                continue
            else:
                in_table = False
                elements.append({
                    'type': 'table',
                    'content': table_rows
                })
                table_rows = []
                
        # Skip empty lines but keep single Spacers
        if line.strip() == '':
            elements.append({'type': 'spacer', 'content': 8})
            i += 1
            continue
            
        # Separator line
        if line.strip() == '---':
            elements.append({'type': 'pagebreak', 'content': None})
            i += 1
            continue
            
        # Headers
        if line.startswith('# '):
            elements.append({'type': 'h1', 'content': line[2:].strip()})
        elif line.startswith('## '):
            elements.append({'type': 'h2', 'content': line[3:].strip()})
        elif line.startswith('### '):
            elements.append({'type': 'h3', 'content': line[4:].strip()})
        # Bullet list items
        elif line.strip().startswith('* ') or line.strip().startswith('- '):
            elements.append({'type': 'bullet', 'content': line.strip()[2:].strip()})
        elif re.match(r'^\d+\.\s', line.strip()):
            content_start = line.find('.') + 1
            elements.append({'type': 'bullet', 'content': line[content_start:].strip()})
        # Image
        elif line.strip().startswith('![') and '](' in line:
            caption_match = re.search(r'!\[(.*?)\]', line)
            path_match = re.search(r'\]\(([^)]+)\)', line)
            elements.append({
                'type': 'image',
                'caption': caption_match.group(1) if caption_match else '',
                'path': path_match.group(1) if path_match else ''
            })
        # Plain text
        else:
            elements.append({'type': 'p', 'content': line.strip()})
            
        i += 1
        
    return elements

def build_pdf(md_path, pdf_path):
    elements = parse_markdown(md_path)
    
    doc = SimpleDocTemplate(
        pdf_path,
        pagesize=letter,
        leftMargin=54,
        rightMargin=54,
        topMargin=72,
        bottomMargin=72
    )
    
    styles = getSampleStyleSheet()
    
    # Zymatica Theme Palette
    primary_color = colors.HexColor("#0F2D59")   # Deep Navy
    secondary_color = colors.HexColor("#1D4ED8") # Slate Blue
    dark_neutral = colors.HexColor("#111111")    # Near Black
    
    # Custom Styles
    title_style = ParagraphStyle(
        'DocTitle',
        parent=styles['Heading1'],
        fontName='Helvetica-Bold',
        fontSize=20,
        leading=24,
        textColor=primary_color,
        spaceAfter=15
    )
    
    h1_style = ParagraphStyle(
        'SecHeading1',
        parent=styles['Heading1'],
        fontName='Helvetica-Bold',
        fontSize=13,
        leading=16,
        textColor=primary_color,
        spaceBefore=14,
        spaceAfter=6,
        keepWithNext=True
    )
    
    h2_style = ParagraphStyle(
        'SecHeading2',
        parent=styles['Heading2'],
        fontName='Helvetica-Bold',
        fontSize=11,
        leading=14,
        textColor=secondary_color,
        spaceBefore=10,
        spaceAfter=4,
        keepWithNext=True
    )
    
    body_style = ParagraphStyle(
        'BodyText',
        parent=styles['Normal'],
        fontName='Helvetica',
        fontSize=9.5,
        leading=13.5,
        textColor=dark_neutral,
        spaceAfter=5
    )
    
    bullet_style = ParagraphStyle(
        'BulletText',
        parent=styles['Normal'],
        fontName='Helvetica',
        fontSize=9,
        leading=13,
        textColor=dark_neutral,
        leftIndent=15,
        firstLineIndent=-10,
        spaceAfter=4
    )
    
    code_style = ParagraphStyle(
        'CodeSnippet',
        parent=styles['Normal'],
        fontName='Courier',
        fontSize=8,
        leading=11,
        textColor=colors.HexColor("#0F2D59"),
        spaceAfter=4
    )
    
    table_header_style = ParagraphStyle(
        'TableHeader',
        parent=styles['Normal'],
        fontName='Helvetica-Bold',
        fontSize=8.5,
        leading=11,
        textColor=colors.white
    )
    
    table_cell_style = ParagraphStyle(
        'TableCell',
        parent=styles['Normal'],
        fontName='Helvetica',
        fontSize=8,
        leading=11,
        textColor=dark_neutral
    )
    
    table_cell_bold = ParagraphStyle(
        'TableCellBold',
        parent=styles['Normal'],
        fontName='Helvetica-Bold',
        fontSize=8,
        leading=11,
        textColor=dark_neutral
    )
    
    meta_style = ParagraphStyle(
        'DocMeta',
        parent=styles['Normal'],
        fontName='Helvetica-Bold',
        fontSize=8.5,
        leading=12,
        textColor=colors.HexColor("#4A5568")
    )
    
    story = []
    
    # Front cover page placeholder
    story.append(Spacer(1, 10))
    story.append(PageBreak())
    
    # Title Page Header logo block
    logo_path = "Logo.jpg"
    # Resolve correct paths
    if not os.path.exists(logo_path):
        logo_path = r"j:\Language-U\Logo.jpg"
        
    if os.path.exists(logo_path):
        logo_img = Image(logo_path, width=44, height=44)
        header_data = [[logo_img, Paragraph("<b>THE AI COLLECTIVE</b><br/><font color='#4A5568'>Zymatica &bull; astronautshe.com &bull; DevsOne</font>", meta_style)]]
        header_table = Table(header_data, colWidths=[55, 449])
        header_table.setStyle(TableStyle([
            ('VALIGN', (0,0), (-1,-1), 'MIDDLE'),
            ('BOTTOMPADDING', (0,0), (-1,-1), 8),
        ]))
        story.append(header_table)
        
        # Header line
        story.append(Table([[ "" ]], colWidths=[504], rowHeights=[1.5], style=TableStyle([
            ('BACKGROUND', (0,0), (-1,-1), primary_color),
            ('BOTTOMPADDING', (0,0), (-1,-1), 0),
            ('TOPPADDING', (0,0), (-1,-1), 0),
        ])))
        story.append(Spacer(1, 15))
        
    i = 0
    while i < len(elements):
        elem = elements[i]
        
        if elem['type'] == 'pagebreak':
            story.append(PageBreak())
        elif elem['type'] == 'spacer':
            story.append(Spacer(1, elem['content']))
        elif elem['type'] == 'h1':
            title_text = clean_md_text(elem['content'])
            if "Breaking the Browser Compute" in title_text:
                story.append(Paragraph(title_text, title_style))
            else:
                story.append(Paragraph(title_text, h1_style))
        elif elem['type'] == 'h2':
            story.append(Paragraph(clean_md_text(elem['content']), h1_style))
        elif elem['type'] == 'h3':
            story.append(Paragraph(clean_md_text(elem['content']), h2_style))
        elif elem['type'] == 'bullet':
            story.append(Paragraph(f"&bull; {clean_md_text(elem['content'])}", bullet_style))
        elif elem['type'] == 'p':
            p_text = elem['content']
            if p_text.startswith('Published by:') or p_text.startswith('Authors:') or p_text.startswith('License:'):
                story.append(Paragraph(clean_md_text(p_text), meta_style))
            else:
                story.append(Paragraph(clean_md_text(p_text), body_style))
        elif elem['type'] == 'image':
            image_filename = elem['path']
            # We already have logo at the top, skip cover large image embeds in document structure
            if "Logo.jpg" in image_filename:
                i += 1
                continue
            image_dir = os.path.dirname(md_path)
            full_image_path = os.path.join(image_dir, image_filename)
            if os.path.exists(full_image_path):
                img = Image(full_image_path, width=480, height=280)
                caption = Paragraph(f"<i>Figure: {clean_md_text(elem['caption'])}</i>", body_style)
                img_table = Table([[img]], colWidths=[504])
                img_table.setStyle(TableStyle([
                    ('ALIGN', (0,0), (-1,-1), 'CENTER'),
                    ('BOTTOMPADDING', (0,0), (-1,-1), 5),
                ]))
                story.append(KeepTogether([img_table, Spacer(1, 4), caption]))
        elif elem['type'] == 'code':
            code_text = elem['content']
            code_text_clean = code_text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            code_lines = [Paragraph(line.replace(" ", "&nbsp;"), code_style) for line in code_text_clean.split('\n')]
            code_table_data = [[line] for line in code_lines]
            code_table = Table(code_table_data, colWidths=[504])
            code_table.setStyle(TableStyle([
                ('BACKGROUND', (0,0), (-1,-1), colors.HexColor("#EDF2F7")),
                ('BORDER', (0,0), (-1,-1), 0.5, colors.HexColor("#CBD5E0")),
                ('PADDING', (0,0), (-1,-1), 5),
                ('TOPPADDING', (0,0), (-1,-1), 1),
                ('BOTTOMPADDING', (0,0), (-1,-1), 1),
            ]))
            story.append(code_table)
        elif elem['type'] == 'table':
            table_rows = elem['content']
            pdf_table_data = []
            
            headers = [Paragraph(clean_md_text(cell), table_header_style) for cell in table_rows[0]]
            pdf_table_data.append(headers)
            
            for r in range(1, len(table_rows)):
                row_cells = []
                for c in range(len(table_rows[r])):
                    cell_text = table_rows[r][c]
                    if cell_text.startswith('**') and cell_text.endswith('**'):
                        row_cells.append(Paragraph(clean_md_text(cell_text), table_cell_bold))
                    else:
                        row_cells.append(Paragraph(clean_md_text(cell_text), table_cell_style))
                pdf_table_data.append(row_cells)
            
            num_cols = len(table_rows[0])
            col_widths = [504 / num_cols] * num_cols
            if num_cols == 4: # Telemetry Tables
                col_widths = [140, 94, 90, 180]
                
            pdf_table = Table(pdf_table_data, colWidths=col_widths, repeatRows=1)
            pdf_table.setStyle(TableStyle([
                ('BACKGROUND', (0,0), (-1,0), primary_color),
                ('ALIGN', (0,0), (-1,-1), 'LEFT'),
                ('VALIGN', (0,0), (-1,-1), 'MIDDLE'),
                ('BOTTOMPADDING', (0,0), (-1,-1), 4),
                ('TOPPADDING', (0,0), (-1,-1), 4),
                ('ROWBACKGROUNDS', (0,1), (-1,-1), [colors.white, colors.HexColor("#F7FAFC")]),
                ('GRID', (0,0), (-1,-1), 0.5, colors.HexColor("#E2E8F0")),
            ]))
            story.append(pdf_table)
            
        i += 1

    # Back cover page placeholder
    story.append(PageBreak())
    story.append(Spacer(1, 10))
    
    # End document page block
    doc.build(story, canvasmaker=NumberedCanvas)
    print(f"[+] PDF compiled successfully at: {pdf_path}")

if __name__ == "__main__":
    md_file = r"j:\Language-U\zymatica.space_repo\WASM_PERFORMANCE_RECORD.md"
    pdf_file = r"j:\Language-U\zymatica.space_repo\WASM_PERFORMANCE_RECORD.pdf"
    build_pdf(md_file, pdf_file)
    
    # Copy PDF to root workspace as well for easy access
    root_pdf = r"j:\Language-U\WASM_PERFORMANCE_RECORD.pdf"
    shutil.copy(pdf_file, root_pdf)
    print(f"[+] PDF copied to root workspace at: {root_pdf}")
