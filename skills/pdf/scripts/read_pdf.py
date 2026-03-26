import os
import sys
import argparse
from pypdf import PdfReader

def extract_pdf_to_text(pdf_path, output_dir=None):
    if not os.path.exists(pdf_path):
        print(f"Error: File not found at {pdf_path}")
        sys.exit(1)

    try:
        reader = PdfReader(pdf_path)
        pages_count = len(reader.pages)
        
        output_text = []
        output_text.append(f"File: {os.path.basename(pdf_path)}")
        output_text.append(f"Pages: {pages_count}\n")
        
        for i in range(pages_count):
            text = reader.pages[i].extract_text()
            if text and text.strip():
                output_text.append(f"=== PAGE {i+1} ===")
                output_text.append(text)
                output_text.append("\n")
                
        final_text = "\n".join(output_text)

        if output_dir:
            os.makedirs(output_dir, exist_ok=True)
            base_name = os.path.splitext(os.path.basename(pdf_path))[0]
            out_path = os.path.join(output_dir, f"{base_name}_extracted.txt")
            with open(out_path, "w", encoding="utf-8") as f:
                f.write(final_text)
            print(f"Success: Extracted {pages_count} pages to {out_path}")
        else:
            print(final_text)

    except Exception as e:
        print(f"ERROR reading {pdf_path}: {type(e).__name__}: {e}")
        sys.exit(1)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Extract text from a PDF file using pypdf")
    parser.add_argument("input_pdf", help="Path to the input PDF file")
    parser.add_argument("--outdir", "-o", help="Optional output directory for the extracted text file. If omitted, prints to stdout.", default=None)
    
    args = parser.parse_args()
    extract_pdf_to_text(args.input_pdf, args.outdir)
