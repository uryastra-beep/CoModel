// Perfil local. No hay cuentas todavía: es solo el nombre con el que se muestra al usuario.
const KEY = "comodel.userName";

function load(): string {
  try {
    return localStorage.getItem(KEY)?.trim() || "Usuario";
  } catch {
    return "Usuario";
  }
}

export const profile = $state({ name: load() });

export function setName(name: string) {
  const n = name.trim().slice(0, 30);
  if (!n) return;
  profile.name = n;
  try {
    localStorage.setItem(KEY, n);
  } catch {
    // Sin almacenamiento: el nombre solo dura esta sesión.
  }
}

// Foto de perfil: se reduce a 256 px y se guarda como imagen en este equipo.
const PHOTO_KEY = "comodel.userPhoto";

function loadPhoto(): string {
  try {
    return localStorage.getItem(PHOTO_KEY) ?? "";
  } catch {
    return "";
  }
}

export const photo = $state({ src: loadPhoto() });

export async function setPhoto(file: File | null) {
  if (!file) {
    photo.src = "";
    try {
      localStorage.removeItem(PHOTO_KEY);
    } catch {
      // sin almacenamiento
    }
    return;
  }
  if (!file.type.startsWith("image/")) throw new Error("Elige un archivo de imagen.");
  const bmp = await createImageBitmap(file);
  const side = Math.min(bmp.width, bmp.height);
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 256;
  // Recorte cuadrado centrado.
  canvas.getContext("2d")!.drawImage(bmp, (bmp.width - side) / 2, (bmp.height - side) / 2, side, side, 0, 0, 256, 256);
  bmp.close();
  photo.src = canvas.toDataURL("image/jpeg", 0.85);
  try {
    localStorage.setItem(PHOTO_KEY, photo.src);
  } catch {
    // sin almacenamiento: la foto solo dura esta sesión
  }
}
