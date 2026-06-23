import gdb
import os

def main():
    input_redirect = os.environ.get("ARGO_INPUT_REDIRECT", "/dev/null")
    bt_limit = int(os.environ.get("ARGO_BT_LIMIT", "15"))

    gdb.execute("set pagination off")
    gdb.execute("set print address off")

    try:
        gdb.execute(f"run < {input_redirect} > /dev/null 2>&1")
    except gdb.error as e:
        print(f"@@ARGO_REASON@@{e}")

    try:
        frame = gdb.newest_frame()
        is_first = True
        count = 0
        
        while frame and count < bt_limit:
            name = frame.name() or "??"
            sal = frame.find_sal()
            code = ""
            
            if sal and sal.symtab:
                file = sal.symtab.filename
                line = sal.line
                if is_first and line > 0:
                    try:
                        with open(sal.symtab.fullname(), 'r', encoding='utf-8') as f:
                            lines = f.readlines()
                            if line <= len(lines):
                                code = lines[line-1].strip()
                    except Exception:
                        pass
            else:
                file = "??"
                line = 0
                
            print(f"@@ARGO_FRAME@@{name}@@{file}@@{line}@@{code}")
            is_first = False
            frame = frame.older()
            count += 1
    except Exception:
        pass

main()
