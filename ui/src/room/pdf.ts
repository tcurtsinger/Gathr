// PDF.js and its worker, loaded only when a PDF's tab opens (PdfBody imports this file on demand).
import { GlobalWorkerOptions, getDocument } from 'pdfjs-dist';
import worker from 'pdfjs-dist/build/pdf.worker.min.mjs?url';

GlobalWorkerOptions.workerSrc = worker;

export { getDocument };
