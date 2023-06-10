import pathlib
import subprocess
import asyncio
import os
import logging

import decky_plugin

HOME_DIR = str(pathlib.Path(os.getcwd()).parent.parent.resolve())
PARENT_DIR = str(pathlib.Path(__file__).parent.resolve())

logging.basicConfig(filename=decky_plugin.DECKY_PLUGIN_LOG,
                    format='[Wine Cellar] %(asctime)s %(levelname)s %(message)s',
                    filemode='w',
                    force=True)

logger = logging.getLogger()
logger.setLevel(logging.DEBUG)
logging.info(f"Wine Cellar main.py https://github.com/FlashyReese/decky-wine-cellar")


class Plugin:
    backend_proc = None

    # Asyncio-compatible long-running code, executed in a task when the plugin is loaded
    async def _main(self):
        # startup with my_env
        os.environ["DECKY_PLUGIN_LOG"] = decky_plugin.DECKY_PLUGIN_LOG
        logger.info("Starting Wine Cask (the Wine Cellar backend)...")
        self.backend_proc = subprocess.Popen([PARENT_DIR + "/bin/backend"])
        while True:
            await asyncio.sleep(1)

    # Function called first during the unload process, utilize this to handle your plugins being removed
    async def _unload(self):
        if self.backend_proc is not None:
            logger.info("Killing Wine Cask (the Wine Cellar backend)...")
            self.backend_proc.kill()
        pass
