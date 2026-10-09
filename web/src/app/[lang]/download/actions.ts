"use server";

import { createHash } from "node:crypto";
import { clientAddress } from "@/lib/request";
import { passedTurnstile } from "@/lib/turnstile";

export type DownloadState = { status: "idle" } | { status: "ready"; url: string } | { status: "error" };

// Where nginx serves the installer (package-client.bat makes GravakSetup.exe).
const INSTALLER = "/downloads/GravakSetup.exe";
// Long enough for a slow connection to finish (or resume) the download.
const LINK_SECONDS = 6 * 60 * 60;

/**
 * After the Turnstile check, a link to the installer signed for nginx's
 * secure_link: md5 of "<expires><uri> <DOWNLOAD_LINK_SECRET>" in base64url,
 * the same string as `secure_link_md5` in the nginx site. Without the secret
 * (developing) the link goes unsigned.
 */
export async function requestDownload(_previous: DownloadState, form: FormData): Promise<DownloadState> {
  if (!(await passedTurnstile(form.get("cf-turnstile-response"), await clientAddress()))) {
    return { status: "error" };
  }
  const secret = process.env.DOWNLOAD_LINK_SECRET;
  if (!secret) return { status: "ready", url: INSTALLER };
  const expires = Math.floor(Date.now() / 1000) + LINK_SECONDS;
  const md5 = createHash("md5").update(`${expires}${INSTALLER} ${secret}`).digest("base64url");
  return { status: "ready", url: `${INSTALLER}?md5=${md5}&expires=${expires}` };
}
