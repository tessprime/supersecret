import argparse
import getpass
import sys

import supersecret as secrets

def main(args):
    parser = argparse.ArgumentParser("Secrets command line interface")
    parser.add_argument("command", choices=["dump", "list", "store", "show", "clear"])
    options, args = parser.parse_known_args(args)

    if options.command == "dump":
        secrets.dumpSecrets()
    elif options.command == "list":
        secrets.listScopes()
    elif options.command == "store":
        if len(args) == 2:
            # Typed at a hidden prompt, so the value stays out of shell history and ps.
            value = getpass.getpass("Value for %s/%s: " % tuple(args))
            if not value:
                print("Nothing typed; nothing stored.")
                return
            secrets.storeSecret(args[0], args[1], value)
        elif len(args) == 3:
            secrets.storeSecret(*args)
        else:
            print("store takes NAMESPACE KEY (and asks for the value), or NAMESPACE KEY VALUE")
    elif options.command == "show":
        if len(args) == 2:
            print(secrets.getSecret(*args))
        if len(args) == 1:
            secrets.dumpScope(*args)
    elif options.command == "clear":
        if len(args) != 1:
            print("clear takes the NAMESPACE to remove")
            return
        secrets.removeAllSecrets(args[0])

if __name__=="__main__":
    main(sys.argv[1:])
