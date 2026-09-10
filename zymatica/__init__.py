# -*- coding: utf-8 -*-
# Zymatica Unified Semantic Intelligence & Inference SDK
# Author: Danny Bouldiez | Codebase: Devs One

import struct
import math

__version__ = "1.0.0"

class Concept8D:
    def __init__(self, domain=0, subdomain=0, operation=0, modality=0, strength=0, polarity=0, temporal=0, certainty=0):
        self.domain = domain & 0x0F
        self.subdomain = subdomain & 0x0F
        self.operation = operation & 0x0F
        self.modality = modality & 0x0F
        self.strength = strength & 0x0F
        self.polarity = polarity & 0x0F
        self.temporal = temporal & 0x0F
        self.certainty = certainty & 0x0F

    def to_dword(self) -> int:
        rc = (self.domain << 4) | self.subdomain
        rf = (self.operation << 4) | self.modality
        ra = (self.strength << 4) | self.polarity
        rt = (self.temporal << 4) | self.certainty
        return (rc << 24) | (rf << 16) | (ra << 8) | rt

    @classmethod
    def from_dword(cls, dword: int):
        rc = (dword >> 24) & 0xFF
        rf = (dword >> 16) & 0xFF
        ra = (dword >> 8) & 0xFF
        rt = dword & 0xFF
        return cls(
            domain=(rc >> 4) & 0x0F,
            subdomain=rc & 0x0F,
            operation=(rf >> 4) & 0x0F,
            modality=rf & 0x0F,
            strength=(ra >> 4) & 0x0F,
            polarity=ra & 0x0F,
            temporal=(rt >> 4) & 0x0F,
            certainty=rt & 0x0F,
        )

class EpigeneticCrystal:
    def __init__(self, domain: int, rank: int, weights: list, hash_val: int):
        self.domain = domain
        self.rank = rank
        self.weights = weights
        self.hash_val = hash_val

    def to_bytes(self) -> bytes:
        head = struct.pack("!BB", self.domain, self.rank)
        w_bytes = struct.pack("!16f", *self.weights)
        tail = struct.pack("!I", self.hash_val)
        return head + w_bytes + tail

    @classmethod
    def project_nullspace(cls, base_activations: list, new_concept: list) -> list:
        dot_prod = sum(a * c for a, c in zip(base_activations, new_concept))
        base_norm_sq = sum(a * a for a in base_activations)
        scalar = dot_prod / base_norm_sq if base_norm_sq > 0 else 0.0
        return [c - scalar * a for a, c in zip(base_activations, new_concept)]

class SwarmIntentChirp:
    def __init__(self, sender: int, epoch: int, domain: int, subdomain: int, opcode: int, coords: list):
        self.sender = sender
        self.epoch = epoch
        self.domain = domain
        self.subdomain = subdomain
        self.opcode = opcode
        self.coords = coords

    def to_bytes(self) -> bytes:
        b2 = (self.domain << 4) | (self.subdomain & 0x0F)
        raw_head = struct.pack("!BBBBB6B", self.sender, self.epoch, b2, self.opcode, 100, *self.coords)
        crc = 0x811c9dc5
        for b in raw_head:
            crc ^= b
            crc = (crc * 0x01000193) & 0xFFFFFFFF
        return raw_head + struct.pack("!IB", crc, 0x5A)

__all__ = ["Concept8D", "EpigeneticCrystal", "SwarmIntentChirp"]
