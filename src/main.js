import { invoke, convertFileSrc } from "@tauri-apps/api/core";

let images = [];
let currentIndex = -1;
let toastTimer = null;

const photo = document.getElementById("photo");
const filenameEl = document.getElementById("filename");
const counterEl = document.getElementById("counter");
const noImageEl = document.getElementById("no-image");
const toastEl = document.getElementById("toast");

function showToast(message, duration = 2000) {
  if (toastTimer) clearTimeout(toastTimer);
  toastEl.textContent = message;
  toastEl.classList.add("show");
  toastTimer = setTimeout(() => toastEl.classList.remove("show"), duration);
}

function getBasename(filePath) {
  return filePath.replace(/\\/g, "/").split("/").pop();
}

function getFolder(filePath) {
  const normalized = filePath.replace(/\\/g, "/");
  return normalized.substring(0, normalized.lastIndexOf("/"));
}

async function displayImage(filePath) {
  photo.classList.remove("loaded");
  const url = convertFileSrc(filePath);

  await new Promise((resolve, reject) => {
    photo.onload = resolve;
    photo.onerror = reject;
    photo.src = url;
  });

  photo.classList.add("loaded");
  noImageEl.style.display = "none";

  const name = getBasename(filePath);
  filenameEl.textContent = name;
  counterEl.textContent = `${currentIndex + 1} / ${images.length}`;
  document.title = `${name} — Photo Viewer`;
}

async function navigate(delta) {
  if (images.length === 0) return;
  currentIndex = ((currentIndex + delta) % images.length + images.length) % images.length;
  try {
    await displayImage(images[currentIndex]);
  } catch {
    showToast("Failed to load image");
  }
}

async function trashCurrent() {
  if (images.length === 0 || currentIndex === -1) return;

  const pathToDelete = images[currentIndex];
  const name = getBasename(pathToDelete);

  images.splice(currentIndex, 1);

  try {
    await invoke("trash_file", { path: pathToDelete });
  } catch (err) {
    images.splice(currentIndex, 0, pathToDelete);
    showToast(`Could not delete: ${err}`);
    return;
  }

  showToast(`Moved to trash: ${name}`);

  if (images.length === 0) {
    photo.classList.remove("loaded");
    photo.src = "";
    filenameEl.textContent = "";
    counterEl.textContent = "";
    noImageEl.style.display = "block";
    document.title = "Photo Viewer";
    currentIndex = -1;
    return;
  }

  if (currentIndex >= images.length) {
    currentIndex = images.length - 1;
  }

  try {
    await displayImage(images[currentIndex]);
  } catch {
    showToast("Failed to load next image");
  }
}

document.addEventListener("keydown", async (e) => {
  if (e.repeat) return;
  switch (e.key) {
    case "ArrowRight":
    case "ArrowDown":
      e.preventDefault();
      await navigate(1);
      break;
    case "ArrowLeft":
    case "ArrowUp":
      e.preventDefault();
      await navigate(-1);
      break;
    case "Delete":
      await trashCurrent();
      break;
  }
});

async function initialize() {
  const initialFile = await invoke("get_initial_file");

  if (!initialFile) {
    noImageEl.style.display = "block";
    return;
  }

  const folder = getFolder(initialFile);
  images = await invoke("get_images_in_folder", { folder });

  if (images.length === 0) {
    noImageEl.style.display = "block";
    return;
  }

  const normalizedInitial = initialFile.replace(/\\/g, "/").toLowerCase();
  currentIndex = images.findIndex(
    (img) => img.replace(/\\/g, "/").toLowerCase() === normalizedInitial
  );
  if (currentIndex === -1) currentIndex = 0;

  try {
    await displayImage(images[currentIndex]);
  } catch {
    showToast("Failed to load image");
  }
}

initialize();
