import { open } from '@tauri-apps/plugin-shell';
import { notifyError } from './notify';

export async function openUrl(url: string): Promise<void> {
  try {
    await open(url);
  } catch (err) {
    notifyError(`Failed to open ${url}`, err);
  }
}

export async function openPath(path: string): Promise<void> {
  try {
    await open(path);
  } catch (err) {
    notifyError(`Failed to open ${path}`, err);
  }
}
