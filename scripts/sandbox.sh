#!/usr/bin/env sh
# Create an isolated dnt sandbox (config, a 2024 campaign and a sample
# multiclass character) for manual testing without touching real data.
#
#   scripts/sandbox.sh DIR [DATA_DIR]
#   XDG_DATA_HOME=DIR/data XDG_CONFIG_HOME=DIR/config cargo run -p dnt
set -eu

dir=${1:?usage: sandbox.sh DIR [DATA_DIR]}
data=${2:-${DNT_DATA:-$HOME/Projects/third-party/dnd-data}}
campaign="$dir/data/dnt/campaigns/curse-of-strahd"

mkdir -p "$dir/config/dnt" "$campaign/characters" "$campaign/notes"

cat > "$dir/config/dnt/config.toml" <<EOF
last_campaign = "curse-of-strahd"

[library]
path = "$data"
EOF

cat > "$campaign/campaign.toml" <<'EOF'
name = "Curse of Strahd"
sources = ["XDMG", "XMM", "XPHB"]
EOF

cat > "$campaign/characters/thalia.toml" <<'EOF'
name = "Thalia Brightshield"
species = "race:Human|XPHB"
background = "background:Soldier|XPHB"
feats = ["feat:Alert|XPHB"]

[abilities]
str = 15
dex = 10
con = 14
int = 8
wis = 12
cha = 15

[[ability_bonus]]
origin = "Background (Soldier)"
str = 2
con = 1

[proficiencies]
skills = ["athletics", "intimidation", "persuasion", "religion"]

[[levels]]
class = "class:Paladin|XPHB"
[[levels]]
class = "class:Paladin|XPHB"
[[levels]]
class = "class:Paladin|XPHB"
[[levels]]
class = "class:Warlock|XPHB"
[[levels]]
class = "class:Warlock|XPHB"

[subclasses]
"class:Paladin|XPHB" = "subclass:Glory|Paladin|XPHB|XPHB"

[equipped]
armor = "item:Chain Mail|XPHB"
shield = true

[state]
hp = 31
temp_hp = 4
resources_used = { lay_on_hands = 6, channel_divinity_paladin = 1 }
slots_used = [1]
EOF

cat > "$campaign/characters/lyra.toml" <<'EOF'
name = "Lyra"
species = "race:Elf; Drow Lineage|XPHB"
background = "background:Acolyte|XPHB"
feats = ["feat:Magic Initiate; Cleric|XPHB"]

[abilities]
str = 8
dex = 14
con = 13
int = 15
wis = 12
cha = 10

[[ability_bonus]]
origin = "Background (Acolyte)"
int = 2
wis = 1

[proficiencies]
skills = ["arcana", "history", "insight", "perception", "religion"]

[[levels]]
class = "class:Wizard|XPHB"
[[levels]]
class = "class:Wizard|XPHB"
[[levels]]
class = "class:Wizard|XPHB"

[subclasses]
"class:Wizard|XPHB" = "subclass:Evoker|Wizard|XPHB|XPHB"
EOF

echo "sandbox ready: XDG_DATA_HOME=$dir/data XDG_CONFIG_HOME=$dir/config"
