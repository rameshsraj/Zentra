#!/usr/bin/env python3
import bcrypt
import subprocess

password = b"Admin@12345"
hashed = bcrypt.hashpw(password, bcrypt.gensalt(12)).decode()
print(f"Generated hash: {hashed}")

# Update in DB
sql = f"UPDATE users SET password_hash = '{hashed}', is_locked = false, locked_until = NULL WHERE email = 'admin@zentra.local';"
result = subprocess.run(
    ["psql", "-h", "10.88.11.162", "-p", "5434", "-U", "postgres", "-d", "zentra", "-c", sql],
    env={"PGPASSWORD": "zantaz", "PATH": "/usr/bin:/usr/local/bin"},
    capture_output=True, text=True
)
print(result.stdout)
if result.returncode != 0:
    print(f"Error: {result.stderr}")
else:
    print("Admin password reset to: Admin@12345")
