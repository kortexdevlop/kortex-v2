import init, { burn_cpu, convert_png_to_jpg } from './pkg/web_worker_demo.js';

await init();
postMessage({ type: 'ready', offMainThread: typeof document === 'undefined' });

onmessage = ({ data }) => {
  try {
    if (data.type === 'convert') {
      const bytes = convert_png_to_jpg(new Uint8Array(data.bytes));
      postMessage({ type: 'converted', bytes: bytes.buffer }, [bytes.buffer]);
    } else if (data.type === 'stress') {
      postMessage({ type: 'started' });
      const checksum = burn_cpu(data.iterations);
      postMessage({ type: 'finished', checksum });
    }
  } catch (error) {
    postMessage({ type: 'error', message: String(error) });
  }
};
