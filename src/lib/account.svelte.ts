// Estado de la cuenta. Iniciar sesión es opcional: sin cuenta la app funciona igual, en local.
import { accountSignOut, accountStatus, type Account } from "./backend";
import { setName } from "./profile.svelte";

export const account = $state<{ user: Account | null; configured: boolean; ready: boolean; dialog: boolean }>({
  user: null,
  configured: false,
  ready: false,
  dialog: false,
});

const SEEN = "comodel.loginOffered";

export function adopt(user: Account | null) {
  account.user = user;
  // El nombre de la cuenta pasa a ser el del perfil; si no lo trae, se usa lo de antes del @.
  if (user) setName(user.name ?? user.email.split("@")[0]);
}

export async function loadAccount() {
  try {
    const s = await accountStatus();
    account.configured = s.configured;
    adopt(s.account);
  } catch {
    // Sin conexión: se sigue como estaba.
  }
  account.ready = true;
  // La primera vez se ofrece una sola vez; "Ahora no" la deja para después.
  let offered = true;
  try {
    offered = localStorage.getItem(SEEN) === "1";
    localStorage.setItem(SEEN, "1");
  } catch {
    // sin almacenamiento
  }
  if (!offered && account.configured && !account.user) account.dialog = true;
}

export async function signOut() {
  await accountSignOut();
  account.user = null;
}

export const openLogin = () => (account.dialog = true);
