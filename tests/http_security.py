#!/usr/bin/env python3
"""Integration checks for the generated Rust HTTP parser."""
import os, socket, subprocess, tempfile, time
from pathlib import Path
root=Path(__file__).resolve().parents[1]
phix=root/'target/release/phix'
with tempfile.TemporaryDirectory() as d:
    deploy=Path(d)
    subprocess.run([str(phix),'build',str(root/'examples/web'),'-o',str(deploy/'app')],check=True,stdout=subprocess.DEVNULL)
    (deploy/'config.json').write_text('{"runtime":{"bind":"127.0.0.1","port":19519}}')
    proc=subprocess.Popen([str(deploy/'app')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    try:
        for _ in range(100):
            try:
                with socket.create_connection(('127.0.0.1',19519),timeout=.2): break
            except OSError: time.sleep(.05)
        def check(payload,expected):
            with socket.create_connection(('127.0.0.1',19519),timeout=3) as s:
                s.sendall(payload); s.shutdown(socket.SHUT_WR)
                result=s.recv(4096).split(b'\r\n',1)[0]
                assert f' {expected} '.encode() in result,(expected,result)
        base=b'GET / HTTP/1.1\r\nHost: localhost\r\n'
        check(base+b'\r\n',200)
        check(b'GET / HTTP/1.1\r\n\r\n',400)
        check(base+b'Host: another\r\n\r\n',400)
        check(base+b'Transfer-Encoding: chunked\r\nContent-Length: 0\r\n\r\n',400)
        check(base+b'Content-Length: 0\r\nContent-Length: 0\r\n\r\n',400)
        check(base+b'X-Test: a\x01b\r\n\r\n',400)
        check(base+b'X-Long: '+b'x'*33000+b'\r\n\r\n',431)
        check(base+(b'X-T: 1\r\n'*101)+b'\r\n',400)
        print('HTTP security checks passed (8 cases)')
    finally:
        proc.terminate();proc.wait(timeout=5)
