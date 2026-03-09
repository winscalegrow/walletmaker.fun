// @ts-ignore
import init, { grind_batch_wasm, validate_prefix_wasm } from '../../public/wasm/solana_vanity';

let initialized = false;

self.onmessage = async (e: MessageEvent) => {
    const { type, prefix, batchSize, origin } = e.data;

    if (type === 'INIT') {
        try {
            const wasmUrl = `${origin}/wasm/solana_vanity_bg.wasm`;
            const response = await fetch(wasmUrl);
            const buffer = await response.arrayBuffer();
            await init({ module_or_path: buffer });
            initialized = true;
            self.postMessage({ type: 'READY' });
        } catch (err) {
            self.postMessage({ type: 'INIT_ERR', error: String(err) });
        }
    }

    if (type === 'GRIND' && initialized) {
        try {
            const result = grind_batch_wasm(prefix, batchSize || 20000);
            if (result) {
                self.postMessage({ type: 'MATCH', data: JSON.parse(result) });
            } else {
                self.postMessage({ type: 'BATCH_DONE' });
            }
        } catch (err) {
            self.postMessage({ type: 'GRIND_ERR', error: String(err) });
        }
    }

    if (type === 'VALIDATE' && initialized) {
        try {
            validate_prefix_wasm(prefix);
            self.postMessage({ type: 'VALIDATE_OK' });
        } catch (err) {
            if (typeof err === "string") {
                self.postMessage({ type: 'VALIDATE_ERR', error: err });
            } else if (err instanceof Error) {
                self.postMessage({ type: 'VALIDATE_ERR', error: err.message });
            } else {
                self.postMessage({ type: 'VALIDATE_ERR', error: String(err) });
            }
        }
    }
};
