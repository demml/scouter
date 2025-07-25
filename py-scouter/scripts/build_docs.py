import os
import shutil

BASE_DIR = os.path.join(os.path.dirname(__file__), "../python/scouter")
FOLDERS = ["queue", "alert", "client", "drift", "profile", "types", "llm"]

for folder in FOLDERS:
    folder_path = os.path.join(BASE_DIR, folder)
    src = os.path.join(folder_path, "__init__.pyi")
    dst = os.path.join(folder_path, f"_{folder}.pyi")
    if os.path.exists(src):
        shutil.copyfile(src, dst)
        print(f"Copied {src} -> {dst}")
    else:
        print(f"Skipped {folder}: {src} does not exist")
