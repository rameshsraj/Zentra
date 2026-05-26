#!/usr/bin/env python3
import subprocess, sys
subprocess.check_call([sys.executable, "-m", "pip", "install", "bcrypt", "-q", "--root-user-action=ignore"], stderr=subprocess.DEVNULL)
import bcrypt
h = bcrypt.hashpw(b"admin123", bcrypt.gensalt(12))
print(h.decode())
