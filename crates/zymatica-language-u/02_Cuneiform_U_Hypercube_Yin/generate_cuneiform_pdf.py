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
        # Running Header
        self.setFont("Helvetica-Bold", 8)
        self.setFillColor(colors.HexColor("#0F2D59"))
        self.drawString(54, 755, "ZYMATICA | CUNEIFORM-U 6D SEMANTIC HYPERCUBE")
        
        self.setFont("Helvetica", 8)
        self.setFillColor(colors.HexColor("#718096"))
        self.drawRightString(558, 755, "CLASS 02: 6D COORDINATE METRIC MANIFOLD")
        
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

def generate_pdf(output_pdf_path="WHITEPAPER.pdf"):
    doc = SimpleDocTemplate(
        output_pdf_path,
        pagesize=letter,
        leftMargin=54,
        rightMargin=54,
        topMargin=60,
        bottomMargin=65
    )
    
    styles = getSampleStyleSheet()
    
    title_style = ParagraphStyle(
        'DocTitle',
        parent=styles['Heading1'],
        fontName='Helvetica-Bold',
        fontSize=18,
        leading=22,
        textColor=colors.HexColor("#0F2D59"),
        spaceAfter=4
    )
    
    subtitle_style = ParagraphStyle(
        'DocSubtitle',
        parent=styles['Normal'],
        fontName='Helvetica-Bold',
        fontSize=9.5,
        leading=13,
        textColor=colors.HexColor("#2B6CB0"),
        spaceAfter=12
    )
    
    h2_style = ParagraphStyle(
        'DocH2',
        parent=styles['Heading2'],
        fontName='Helvetica-Bold',
        fontSize=12,
        leading=16,
        textColor=colors.HexColor("#0F2D59"),
        spaceBefore=12,
        spaceAfter=6
    )
    
    h3_style = ParagraphStyle(
        'DocH3',
        parent=styles['Heading3'],
        fontName='Helvetica-Bold',
        fontSize=10,
        leading=13,
        textColor=colors.HexColor("#2B6CB0"),
        spaceBefore=8,
        spaceAfter=4
    )
    
    body_style = ParagraphStyle(
        'DocBody',
        parent=styles['Normal'],
        fontName='Helvetica',
        fontSize=8.5,
        leading=12.5,
        textColor=colors.HexColor("#2D3748"),
        spaceAfter=6
    )
    
    terminal_text_style = ParagraphStyle(
        'TerminalText',
        parent=styles['Normal'],
        fontName='Courier-Bold',
        fontSize=8,
        leading=11,
        textColor=colors.HexColor("#00FF88"),
        spaceAfter=2
    )

    terminal_subtext_style = ParagraphStyle(
        'TerminalSubText',
        parent=styles['Normal'],
        fontName='Courier',
        fontSize=7.5,
        leading=10.5,
        textColor=colors.HexColor("#A0AEC0"),
        spaceAfter=2
    )
    
    quote_style = ParagraphStyle(
        'DocQuote',
        parent=styles['Normal'],
        fontName='Helvetica-Oblique',
        fontSize=8.5,
        leading=12,
        textColor=colors.HexColor("#1A365D"),
        spaceBefore=4,
        spaceAfter=8
    )
    
    story = []
    
    story.append(Paragraph("ZYMATICA: Cuneiform-U 6D Semantic Hypercube System", title_style))
    story.append(Paragraph("<b>IP Class 02 &nbsp;|&nbsp; 6-Dimensional Semantic Metric Manifold &nbsp;|&nbsp; Zymatica Covenant License 2.0 (zymatica.space)</b>", subtitle_style))
    
    # Terminal Simulation Box
    term_p1 = Paragraph("<b>[ZYMATICA OS // VANCE FORENSIC DRIVE MONITOR // KERNEL v10.0.0]</b>", terminal_text_style)
    term_p2 = Paragraph("<b>KERNEL STATUS: ONLINE  |  AVX-512 VECTOR BUFFER: LOCKED  |  MTU: 3 BYTES</b>", terminal_text_style)
    term_p3 = Paragraph("Decomposition: H(Text) -&gt; H(Meaning) + H(Syntax | Meaning)<br/>Metric Tensor: 6-Dimensional Semantic Metric Hypercube [D, S, O, M, d, P]<br/>Resonance Engine: 26_Perpetual_Motion_Eigenspace_Loops", terminal_subtext_style)
    
    term_table = Table([[term_p1], [term_p2], [Spacer(1, 3)], [term_p3]], colWidths=[500])
    term_table.setStyle(TableStyle([
        ('BACKGROUND', (0,0), (-1,-1), colors.HexColor("#070B14")),
        ('BOX', (0,0), (-1,-1), 1.2, colors.HexColor("#00F0FF")),
        ('PADDING', (0,0), (-1,-1), 8),
        ('VALIGN', (0,0), (-1,-1), 'MIDDLE'),
    ]))
    story.append(term_table)
    story.append(Spacer(1, 10))

    story.append(Paragraph("<i>\"The impossible is just code waiting to be written, physics waiting to be rewritten, math a work in progress, and truth waiting to be discovered.\"</i> — <b>200 Amsterdam: The Vertical City</b>", quote_style))
    story.append(Spacer(1, 6))
    
    story.append(Paragraph("1. Technical Overview &amp; The Entropy Equation", h2_style))
    story.append(Paragraph(
        "\"<i>'My God,' Lindqvist breathed. 'Look at the entropy equation... Shannon was a genius, but in his 1948 foundation paper, he explicitly set semantic meaning aside. Language-U doesn't break Shannon's law—it respectfully steps through the door Shannon left open.'</i>\"",
        quote_style
    ))
    story.append(Paragraph(
        "The <b>Cuneiform-U Semantic Hypercube</b> is a structured coordinate metric space that maps discrete natural language tokens onto a continuous, low-dimensional geometric manifold (<b>R<sup>6</sup></b>). Traditional tokenizers represent items as unstructured integer IDs. Under quantization noise (SVD degradation), the logit distribution drifts and shatters. Cuneiform-U binds all vocabulary items along six orthogonal axes, forcing errors to resolve into geometrically adjacent, semantically valid concepts.",
        body_style
    ))
    
    # 6D Axes Table
    axes_data = [
        [Paragraph("<b>Axis</b>", body_style), Paragraph("<b>Dimension Name</b>", body_style), Paragraph("<b>Range</b>", body_style), Paragraph("<b>Functional Role</b>", body_style)],
        [Paragraph("<b>1</b>", body_style), Paragraph("<b>Domain (D)</b>", body_style), Paragraph("0x0..0xF", body_style), Paragraph("Macro-topic knowledge category (Hardware, Math, Dialogue)", body_style)],
        [Paragraph("<b>2</b>", body_style), Paragraph("<b>Subdomain (S)</b>", body_style), Paragraph("0x0..0xF", body_style), Paragraph("Micro-topic technical context (LoRa RF, SVD, Entropy)", body_style)],
        [Paragraph("<b>3</b>", body_style), Paragraph("<b>Operation (O)</b>", body_style), Paragraph("0x0..0xF", body_style), Paragraph("Functional action / state transition (Query, Compress, Heal)", body_style)],
        [Paragraph("<b>4</b>", body_style), Paragraph("<b>Modality (M)</b>", body_style), Paragraph("0x0..0xF", body_style), Paragraph("Data schema / transport layout (Binary, Radicals, JSON)", body_style)],
        [Paragraph("<b>5</b>", body_style), Paragraph("<b>Depth (d)</b>", body_style), Paragraph("0x0..0xF", body_style), Paragraph("Complexity hierarchy / scale (Seed, Primitive Glyph, AST)", body_style)],
        [Paragraph("<b>6</b>", body_style), Paragraph("<b>Polarity (P)</b>", body_style), Paragraph("0x0..0xF", body_style), Paragraph("Logical direction / confirmation flag (ACK, NACK, Critical)", body_style)],
    ]
    axes_table = Table(axes_data, colWidths=[30, 95, 55, 320])
    axes_table.setStyle(TableStyle([
        ('BACKGROUND', (0,0), (-1,0), colors.HexColor("#EDF2F7")),
        ('BOX', (0,0), (-1,-1), 1, colors.HexColor("#CBD5E0")),
        ('INNERGRID', (0,0), (-1,-1), 0.5, colors.HexColor("#E2E8F0")),
        ('PADDING', (0,0), (-1,-1), 4),
        ('VALIGN', (0,0), (-1,-1), 'MIDDLE'),
    ]))
    story.append(axes_table)
    story.append(Spacer(1, 10))

    story.append(Paragraph("2. 3-Byte Radical Packing Scheme (24-Bit Bitstream)", h2_style))
    story.append(Paragraph(
        "To transmit semantic intent across airgapped, low-power LoRa radios (915 MHz SX1302) without internet, the 6 coordinate nibbles are compressed into three 8-bit <b>Radical Bytes</b>:",
        body_style
    ))
    story.append(Paragraph("• <b>Classifier Radical (R<sub>C</sub>: 1 Byte):</b> Encodes high-level taxonomy: <i>R<sub>C</sub> = (D &lt;&lt; 4) | (S &amp; 0xF)</i>", body_style))
    story.append(Paragraph("• <b>Factor Radical (R<sub>F</sub>: 1 Byte):</b> Encodes system action and modality: <i>R<sub>F</sub> = (O &lt;&lt; 4) | (M &amp; 0xF)</i>", body_style))
    story.append(Paragraph("• <b>Active Radical (R<sub>A</sub>: 1 Byte):</b> Encodes depth hierarchy and polarity: <i>R<sub>A</sub> = (d &lt;&lt; 4) | (P &amp; 0xF)</i>", body_style))
    
    story.append(Spacer(1, 8))
    story.append(Paragraph("3. Adversarial Peer Audit &amp; Defenses", h2_style))
    story.append(Paragraph("<b>Critique 2.1: Semantic Compression Ambiguity (Many-to-One)</b>", h3_style))
    story.append(Paragraph("<b>Defense:</b> Standard flat vocabularies suffer catastrophic collapse under low-rank SVD noise because tokens are treated as independent classes. By embedding tokens in a 6D metric space, the Radical Coordinate Resonance Loss (RCRA) regularizes the model to output semantically adjacent concepts even under extreme lossy quantization. Furthermore, the coordinate bounds allow the S-PAUP GPU router to perform sub-millisecond dynamic JIT adapter swapping.", body_style))
    
    story.append(Paragraph("<b>Critique 2.2: Channel Entropy &amp; Physical Feasibility</b>", h3_style))
    story.append(Paragraph("<b>Defense:</b> Shannon's 1948 Source Coding Theorem addressed syntactic symbol transmission without prior context: <i>H(X) = -&Sigma; P(xᵢ) log₂ P(xᵢ)</i>. Language-U transmits <i>H(Text) = H(Meaning) + H(Syntax | Meaning)</i>. The physical channel carries only the sparse 24-bit trajectory ($H(\\text{Meaning})$), while local generative priors reconstruct the syntax deterministically, achieving 7.2x bandwidth reduction while honoring Shannon's conditional entropy bounds.", body_style))
    
    doc.build(story, canvasmaker=NumberedCanvas)
    print(f"Generated {output_pdf_path} successfully!")

if __name__ == "__main__":
    generate_pdf("WHITEPAPER.pdf")
