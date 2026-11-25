import os
import shutil
import struct
import sys
import tempfile
import winreg
import zlib
from pathlib import Path


def steam_config_dir():
    steam_base = Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / "Steam"
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Valve\Steam") as key:
            steam_base = Path(winreg.QueryValueEx(key, "SteamPath")[0])
    except FileNotFoundError:
        pass

    userdata = steam_base / "userdata"
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Valve\Steam\ActiveProcess") as key:
            active_user = winreg.QueryValueEx(key, "ActiveUser")[0]
        active_config = userdata / str(active_user) / "config"
        if active_user and active_config.is_dir():
            return active_config
    except FileNotFoundError:
        pass

    configs = [account / "config" for account in userdata.glob("*")
               if account.name.isdecimal() and account.name != "0" and (account / "config").is_dir()]
    if len(configs) == 1:
        return configs[0]
    raise RuntimeError("Uruchom Steam i zaloguj się na konto, do którego chcesz dodać skrót.")

def parse_vdf(data, pos=0):
    res = {}
    while pos < len(data):
        t = data[pos]
        pos += 1
        if t == 8:
            break
        k_end = data.find(b'\x00', pos)
        if k_end < 0:
            raise ValueError("Nieprawidłowy plik skrótów Steam.")
        key = data[pos:k_end].decode('utf-8', errors='ignore')
        pos = k_end + 1
        if t == 0:
            val, pos = parse_vdf(data, pos)
            res[key] = (0, val)
        elif t == 1:
            v_end = data.find(b'\x00', pos)
            if v_end < 0:
                raise ValueError("Nieprawidłowy plik skrótów Steam.")
            val = data[pos:v_end].decode('utf-8', errors='ignore')
            pos = v_end + 1
            res[key] = (1, val)
        elif t == 2:
            val = struct.unpack('<i', data[pos:pos+4])[0]
            pos += 4
            res[key] = (2, val)
        else:
            raise ValueError(f"Nieobsługiwany typ danych w pliku skrótów Steam: {t}.")
    return res, pos

def serialize_vdf(obj):
    out = bytearray()
    for k, (t, v) in obj.items():
        out.append(t)
        out.extend(k.encode('utf-8') + b'\x00')
        if t == 0:
            out.extend(serialize_vdf(v))
            out.append(8)
        elif t == 1:
            out.extend(v.encode('utf-8') + b'\x00')
        elif t == 2:
            out.extend(struct.pack('<i', v))
    return bytes(out)

def install_steam_shortcut():
    user_config_dir = steam_config_dir()
    grid_dir = os.path.join(user_config_dir, "grid")
    vdf_path = os.path.join(user_config_dir, "shortcuts.vdf")
    bak_path = os.path.join(user_config_dir, "shortcuts.vdf.bak")

    root = Path(__file__).resolve().parent
    victory_dir = root / "victory"
    game_dir = root / "GTA6.Xbox.v1.0.2.1-CYBERLEEK"
    binary = victory_dir / "target" / "release" / "victory.exe"
    game = game_dir / "game_scarlett_master.exe"
    for required in (binary, game):
        if not required.is_file():
            raise FileNotFoundError(f"Brakuje pliku: {required}")
    binary_path = str(binary)
    # Keep the AppID of shortcuts created by the original launcher.
    if binary.drive.endswith(":"):
        binary_path = binary.drive.lower() + binary_path[len(binary.drive):]
    exe = f'"{binary_path}"'
    name = "Grand Theft Auto VI"
    launch_options = f'run "{game}" --unimplemented zero --console series-s --fps 60 --output-height 1080'
    start_dir = f'"{victory_dir}"'
    ico_path = root / "gta6.ico"

    # Compute AppID
    c = (exe + name).encode('utf-8')
    crc = zlib.crc32(c)
    short_id = crc | 0x80000000
    signed_id = struct.unpack('<i', struct.pack('<I', short_id))[0]
    unsigned_id = short_id & 0xFFFFFFFF
    long_id = (unsigned_id << 32) | 0x02000000

    print(f"[+] Steam AppID (32-bit): {signed_id} (Unsigned: {unsigned_id})")
    print(f"[+] Steam AppID (64-bit): {long_id}")

    # Generate Artwork
    splash_path = game_dir / "xbs_splash.png"
    logo_path = game_dir / "xbs_logo_480.png"
    logo150_path = game_dir / "xbs_logo_150.png"
    try:
        from PIL import Image
    except ImportError:
        Image = None

    if Image and all(path.is_file() for path in (splash_path, logo_path, logo150_path)):
        os.makedirs(grid_dir, exist_ok=True)
        splash = Image.open(splash_path).convert("RGBA")
        logo = Image.open(logo_path).convert("RGBA")
        logo150 = Image.open(logo150_path).convert("RGBA")

        # Icon (.ico)
        logo150.save(ico_path, format="ICO", sizes=[(16,16), (32,32), (48,48), (64,64), (128,128), (150,150)])

        # Logo
        logo.save(os.path.join(grid_dir, f"{unsigned_id}_logo.png"))

        # Hero (1920x620)
        hero = splash.crop((0, 80, 1920, 700))
        hero.save(os.path.join(grid_dir, f"{unsigned_id}_hero.png"))

        # Banner (920x430)
        banner = splash.resize((920, int(920 * 1080 / 1920)), Image.Resampling.LANCZOS)
        b_top = (banner.height - 430) // 2
        banner = banner.crop((0, b_top, 920, b_top + 430))
        banner.save(os.path.join(grid_dir, f"{unsigned_id}.png"))

        # Portrait Capsule (600x900)
        p_crop = splash.crop((550, 0, 1370, 1080)).resize((600, 900), Image.Resampling.LANCZOS)
        logo_small = logo.resize((400, 400), Image.Resampling.LANCZOS)
        p_crop.paste(logo_small, (100, 450), logo_small)
        p_crop.save(os.path.join(grid_dir, f"{unsigned_id}p.png"))
        print("[+] Wygenerowano kompletną grafikę Steam Grid (okładka, baner, tło hero, logo, ikona).")

    # Update shortcuts.vdf
    if not os.path.exists(bak_path) and os.path.exists(vdf_path):
        shutil.copy2(vdf_path, bak_path)

    if os.path.exists(vdf_path):
        with open(vdf_path, 'rb') as f:
            content = f.read()
        parsed, _ = parse_vdf(content)
    else:
        parsed = {'shortcuts': (0, {})}

    shortcuts_map = parsed['shortcuts'][1]

    target_idx = None
    for idx_str, (_, sc) in shortcuts_map.items():
        if sc.get('AppName', (None, None))[1] == 'Grand Theft Auto VI':
            target_idx = idx_str
            break

    if target_idx is None:
        indices = [int(k) for k in shortcuts_map.keys() if k.isdigit()]
        target_idx = str(max(indices) + 1 if indices else 0)

    gta6_entry = {
        'appid': (2, signed_id),
        'AppName': (1, 'Grand Theft Auto VI'),
        'Exe': (1, exe),
        'StartDir': (1, start_dir),
        'icon': (1, str(ico_path)),
        'ShortcutPath': (1, ''),
        'LaunchOptions': (1, launch_options),
        'IsHidden': (2, 0),
        'AllowDesktopConfig': (2, 1),
        'AllowOverlay': (2, 1),
        'OpenVR': (2, 0),
        'Devkit': (2, 0),
        'DevkitGameID': (1, ''),
        'DevkitOverrideAppID': (2, 0),
        'LastPlayTime': (2, 0),
        'FlatpakAppID': (1, ''),
        'sortas': (1, ''),
        'tags': (0, {})
    }

    shortcuts_map[target_idx] = (0, gta6_entry)
    new_bytes = serialize_vdf(parsed) + b'\x08'
    pending = tempfile.NamedTemporaryFile(dir=user_config_dir, delete=False)
    temporary_path = Path(pending.name)
    try:
        with pending:
            pending.write(new_bytes)
        temporary_path.replace(vdf_path)
    finally:
        temporary_path.unlink(missing_ok=True)

    print(f"[+] Zapisano wpis Grand Theft Auto VI do {vdf_path}")

if __name__ == "__main__":
    try:
        install_steam_shortcut()
    except (OSError, RuntimeError, ValueError, struct.error) as error:
        sys.exit(str(error))
