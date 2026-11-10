#!/usr/bin/env python3
"""Código Steam Guard (TOTP de 5 caracteres) a partir do `shared_secret` em base64.

Uso: guard-code.py <shared_secret>. Só stdlib; o CI chama logo antes do `steamcmd +login`,
porque o código vale ~30 s.
"""
import base64
import hashlib
import hmac
import struct
import sys
import time

ALPHABET = "23456789BCDFGHJKMNPQRTVWXY"


def guard_code(shared_secret_b64: str, at: float | None = None) -> str:
    # O secret colado no GitHub costuma perder o `=` final e ganhar espaço/quebra de linha: o
    # `b64decode` recusa base64 sem padding.
    secret = "".join(shared_secret_b64.split())
    key = base64.b64decode(secret + "=" * (-len(secret) % 4))
    counter = int((time.time() if at is None else at) // 30)
    digest = hmac.new(key, struct.pack(">Q", counter), hashlib.sha1).digest()
    offset = digest[19] & 0x0F
    value = struct.unpack(">I", digest[offset : offset + 4])[0] & 0x7FFFFFFF
    chars = []
    for _ in range(5):
        chars.append(ALPHABET[value % len(ALPHABET)])
        value //= len(ALPHABET)
    return "".join(chars)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("uso: guard-code.py <shared_secret_base64>")
    print(guard_code(sys.argv[1]))
