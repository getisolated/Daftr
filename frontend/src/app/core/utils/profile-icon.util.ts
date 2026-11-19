import { PROFILE_ICON_SIZE } from '../models/profile.model';
import { squareCrop } from './profile.util';

/** WebP is lighter; a web view that cannot encode it (WKWebView) hands back a PNG, which the main process accepts too. */
const ICON_TYPE = 'image/webp';
const ICON_QUALITY = 0.9;

/** The image cropped to its centred square and scaled down to the icon size, ready to be sent to the main process. */
export async function renderProfileIcon(image: Blob): Promise<Blob> {
  const bitmap = await createImageBitmap(image);
  const crop = squareCrop(bitmap.width, bitmap.height, PROFILE_ICON_SIZE);
  const canvas = document.createElement('canvas');
  canvas.width = crop.target;
  canvas.height = crop.target;

  const context = canvas.getContext('2d');

  if (!context) {
    bitmap.close();
    throw new Error('Canvas unavailable');
  }

  context.imageSmoothingQuality = 'high';
  context.drawImage(bitmap, crop.sx, crop.sy, crop.size, crop.size, 0, 0, crop.target, crop.target);
  bitmap.close();

  return await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(blob => (blob ? resolve(blob) : reject(new Error('Image not encoded'))), ICON_TYPE, ICON_QUALITY);
  });
}
