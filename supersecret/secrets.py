import json
import os
import tempfile
import shutil

class NoSuchSecret(Exception):
    def __init__(self, scope, key):
        self.scope = scope
        self.key = key
    def __str__(self):
        return "'%s' is not a secret in scope '%s'" % (self.key, self.scope)

def default_path():
    """The store folder: $SUPERSECRET_DIR if set, else ~/.supersecret."""
    return os.environ.get("SUPERSECRET_DIR") or os.path.expanduser("~/.supersecret")

class FileSystemProvider(object):
    def __init__(self, path=None, creds=None):
        if path is None:
            path = default_path()
        if not os.path.exists(path):
            os.makedirs(path, mode=0o700)
        if creds == None:
            creds = credentials
        self.path = path
        self.creds = creds

    def scope_path(self, scope):
        return "%s/%s" % (self.path, scope)

    def getSecrets(self, scope):
        scope_path = self.scope_path(scope)
        if not os.path.exists(scope_path):
            with open(scope_path, "w") as initial:
                initial.write("{}")
        with open(scope_path) as secretFile:
            return json.loads(secretFile.read())

    def hasSecret(self, scope, key):
        return key in self.getSecrets(scope)

    def removeAllSecrets(self, scope):
        if os.path.exists(self.scope_path(scope)):
            os.remove(self.scope_path(scope))

    def getSecret(self, scope, key):
        try:
            return self.getSecrets(scope)[key]
        except KeyError:
            raise NoSuchSecret(scope, key)

    def storeSecret(self, scope, key, value):
        secrets = self.getSecrets(scope)
        secrets[key] = value
        # File swap: a private temporary file in the store's own folder (not
        # /tmp), renamed over the scope, so readers never see half a file.
        # Never print: callers' output may be recorded (agents' transcripts).
        with tempfile.NamedTemporaryFile(mode='w', delete=False, dir=self.path,
                                         prefix="." + scope + ".tmp-") as temp_secret_file:
            json.dump(secrets, temp_secret_file)
        os.replace(temp_secret_file.name, self.scope_path(scope))

    def getScopes(self):
        result = []
        for file in os.listdir(self.path):
            result.append(file)
        return result

    def dumpScope(self, scope):
        print(self.getSecrets(scope))

    def dumpSecrets(self):
        for scope in self.getScopes():
            print(scope)
            self.dumpScope(scope)

credentials = None
provider = FileSystemProvider()

def setProvider(new_provider):
    """Use another provider for the functions below (e.g. a test folder).
    Assigning supersecret.provider doesn't: these read this module's own."""
    global provider
    provider = new_provider

def hasSecret(scope, key):
    return provider.hasSecret(scope, key)

def getSecret(scope, key):
    return provider.getSecret(scope, key)

def removeAllSecrets(scope):
    return provider.removeAllSecrets(scope)

def storeSecret(scope, key, value):
    return provider.storeSecret(scope, key, value)

def setCredentials(creds):
    creds = creds

def dumpSecrets():
    provider.dumpSecrets()

def listScopes():
    for scope in provider.getScopes():
        print(scope)

def dumpScope(scope):
    provider.dumpScope(scope)

def require(secrets):
    for scope in secrets:
        scope_secrets = secrets.getSecrets(scope)
        for key in scope:
            if key not in scope_secrets:
                raise NoSuchSecret(scope, key)
