import pathlib
import subprocess
import asyncio
import os
import logging
from settings import SettingsManager  # type: ignore
import decky_plugin

HOME_DIR = str(pathlib.Path(os.getcwd()).parent.parent.resolve())
PARENT_DIR = str(pathlib.Path(__file__).parent.resolve())

logging.basicConfig(filename=decky_plugin.DECKY_PLUGIN_LOG,
                    format='[Wine Cellar] %(asctime)s %(levelname)s %(message)s',
                    filemode='w',
                    force=True)

logger = logging.getLogger()
logger.setLevel(logging.DEBUG)
logger.info(f"Wine Cellar main.py https://github.com/FlashyReese/decky-wine-cellar")

logger.info('[backend] Settings path: {}'.format(decky_plugin.DECKY_PLUGIN_SETTINGS_DIR))
settings = SettingsManager(name="settings", settings_directory=decky_plugin.DECKY_PLUGIN_SETTINGS_DIR)
settings.read()


class Plugin:
    backend_proc = None

    # Asyncio-compatible long-running code, executed in a task when the plugin is loaded
    async def _main(self):
        logger.info("Starting Wine Cask (the Wine Cellar backend)...")
        self.backend_proc = subprocess.Popen([PARENT_DIR + "/bin/backend"])
        while True:
            await asyncio.sleep(1)

    # Function called first during the unload process, utilize this to handle your plugins being removed
    async def _unload(self):
        if self.backend_proc is not None:
            logger.info("Killing Wine Cask (the Wine Cellar backend)...")
            self.backend_proc.terminate()
            try:
                self.backend_proc.wait(timeout=5)  # 5 seconds timeout
            except subprocess.TimeoutExpired:
                self.backend_proc.kill()
            self.backend_proc = None
        pass

    async def restart_backend(self):
        if self.backend_proc is not None:
            logger.info("Killing Wine Cask (the Wine Cellar backend)...")
            self.backend_proc.terminate()
            try:
                self.backend_proc.wait(timeout=5)  # 5 seconds timeout
            except subprocess.TimeoutExpired:
                self.backend_proc.kill()
            self.backend_proc = None
        logger.info("Starting Wine Cask (the Wine Cellar backend)...")
        self.backend_proc = subprocess.Popen([PARENT_DIR + "/bin/backend"])

    async def settings_read(self):
        logger.info('Reading settings')
        return settings.read()

    async def settings_commit(self):
        logger.info('Saving settings')
        return settings.commit()

    async def settings_getSetting(self, key: str, defaults):
        logger.info('Get {}'.format(key))
        return settings.getSetting(key, defaults)

    async def settings_setSetting(self, key: str, value):
        logger.info('Set {}: {}'.format(key, value))
        return settings.setSetting(key, value)
