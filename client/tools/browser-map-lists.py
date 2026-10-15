#!/usr/bin/env python3
"""Writes the browser client's per-map preload lists, web/maps/<id>.txt
(docs/web-spike.md): each map's own files and its monsters' models.

    browser-map-lists.py <map-files> <monster-files> <spawns> <world.txt> <out dir>

<map-files>      output of `conquer-sandbox --map-files <game>`
<monster-files>  output of `conquer-sandbox --monster-files <game> <look>...`
<spawns>         "<map id> <look>" lines: SELECT DISTINCT s.map_id, t.look FROM
                 monster_spawns s JOIN monster_types t ON t.id = s.monster_type_id
<world.txt>      what the login screens already fetch, left out of every list
"""
import pathlib
import sys


def sections(path, marker):
    """{key: [files]} of a `<marker> <key>` then file-per-line output."""
    out, current = {}, None
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if line.startswith(marker + " "):
            current = out.setdefault(line[len(marker) + 1:], [])
        elif line and current is not None:
            current.append(line)
    return out


def main(map_files, monster_files, spawns, world, out_dir):
    maps = sections(map_files, "MAP")
    looks = sections(monster_files, "LOOK")
    common = {line.strip() for line in open(world, encoding="utf-8") if line.strip() and not line.startswith("#")}
    monsters = {}
    for line in open(spawns, encoding="utf-8"):
        fields = line.split()
        if len(fields) == 2:
            monsters.setdefault(fields[0], set()).add(fields[1])
    out = pathlib.Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    for old in out.glob("*.txt"):
        old.unlink()
    for map_id, files in maps.items():
        listed = list(dict.fromkeys(files))
        for look in sorted(monsters.get(map_id, ()), key=int):
            listed.extend(looks.get(look, []))
        listed = [name for name in dict.fromkeys(listed) if name not in common]
        header = f"# Map {map_id}'s files and its monsters' models (tools/browser-map-lists.py), asked as a hero enters it.\n"
        (out / f"{map_id}.txt").write_text(header + "\n".join(listed) + "\n", encoding="utf-8")
    print(f"{len(maps)} maps, {sum(len(v) for v in monsters.values())} monster looks placed")


if __name__ == "__main__":
    if len(sys.argv) != 6:
        sys.exit(__doc__)
    main(*sys.argv[1:])
