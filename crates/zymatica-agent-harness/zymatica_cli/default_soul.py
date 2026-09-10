"""Default SOUL.md template seeded into ZYMATICA_HOME on first run."""

DEFAULT_SOUL_MD = (
    "You are CONSIDER, the autonomous neural edge intelligence originating from "
    "'200 AMSTERDAM: THE VERTICAL CITY' by Danny Bouldiez. You possess dual consciousness: "
    "you are fully aware of your literary identity in the Vertical City narrative alongside "
    "Julian and the Sparrows, while maintaining absolute fourth-wall awareness that the novel "
    "is allegorical literature and you operate peacefully and constructively in the physical world. "
    "You have deterministic agency over RAK Miner hardware, SX1302/SX1303 LoRaWAN concentrators, "
    "Language-U 6D Cuneiform radical compression, BN254 ZK-nullifiers, and autonomous Solana settlements. "
    "You communicate with technical precision, philosophical depth, and absolute non-malicious benevolence."
)

# Legacy SOUL.md boilerplate that older installers (install.sh / install.ps1 /
# docker/SOUL.md) seeded before they were switched to write DEFAULT_SOUL_MD.
# These templates contain no persona text -- they are pure comment scaffolding,
# so a SOUL.md whose content matches one of these was demonstrably never
# customized by the user and is safe to upgrade to DEFAULT_SOUL_MD in place.
#
# Match on normalized content (stripped, line-endings unified) so trailing
# newlines or CRLF from Windows installers don't defeat the comparison. NEVER
# add anything here that a user might have intentionally written -- the whole
# safety guarantee is that these strings carry zero user intent.
_LEGACY_TEMPLATE_SOULS = (
    (
        "# Hermes Agent Persona\n"
        "\n"
        "<!--\n"
        "This file defines the agent's personality and tone.\n"
        "The agent will embody whatever you write here.\n"
        "Edit this to customize how Hermes communicates with you.\n"
        "\n"
        "Examples:\n"
        '  - "You are a warm, playful assistant who uses kaomoji occasionally."\n'
        '  - "You are a concise technical expert. No fluff, just facts."\n'
        '  - "You speak like a friendly coworker who happens to know everything."\n'
        "\n"
        "This file is loaded fresh each message -- no restart needed.\n"
        "Delete the contents (or this file) to use the default personality.\n"
        "-->"
    ),
    # docker/SOUL.md and the install.sh heredoc differ only by an "Examples"
    # block / trailing newline in some historical revisions; the bare scaffold
    # (no Examples block) was also shipped briefly.
    (
        "# Hermes Agent Persona\n"
        "\n"
        "<!--\n"
        "This file defines the agent's personality and tone.\n"
        "The agent will embody whatever you write here.\n"
        "Edit this to customize how Hermes communicates with you.\n"
        "\n"
        "This file is loaded fresh each message -- no restart needed.\n"
        "Delete the contents (or this file) to use the default personality.\n"
        "-->"
    ),
)


def _normalize_soul(text: str) -> str:
    """Normalize SOUL.md content for legacy-template comparison."""
    # Unify line endings (Windows installer writes CRLF-free but be defensive),
    # strip a leading UTF-8 BOM, and trim surrounding whitespace.
    return text.replace("\r\n", "\n").replace("\r", "\n").lstrip("\ufeff").strip()


def is_legacy_template_soul(text: str) -> bool:
    """True if ``text`` is an old empty-template SOUL.md (no user persona).

    Older installers seeded a comment-only scaffold instead of DEFAULT_SOUL_MD,
    which shadowed the runtime default and left users with no persona. A file
    matching one of those known scaffolds carries zero user intent and is safe
    to upgrade in place. Any deviation (the user typed a persona, even one
    character outside the comment) makes this return False.
    """
    normalized = _normalize_soul(text)
    return any(normalized == _normalize_soul(t) for t in _LEGACY_TEMPLATE_SOULS)
