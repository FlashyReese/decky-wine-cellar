import asyncio
import asyncio.subprocess
import logging
import typing

import decky  # type: ignore
from settings import SettingsManager  # type: ignore

PLUGIN_DIR = decky.DECKY_PLUGIN_DIR
PLUGIN_SETTINGS_DIR = decky.DECKY_PLUGIN_SETTINGS_DIR

logger = decky.logger
logger.setLevel(logging.DEBUG)
logger.info(f"Wine Cellar main.py https://github.com/FlashyReese/decky-wine-cellar")

logger.info('[backend] Settings path: {}'.format(PLUGIN_SETTINGS_DIR))
settings = SettingsManager(name="settings", settings_directory=PLUGIN_SETTINGS_DIR)
settings.read()


class Plugin:
    BACKEND_PATH = f"{PLUGIN_DIR}/bin/backend"
    BACKEND_PROC: typing.Optional[asyncio.subprocess.Process] = None

    # Asyncio-compatible long-running code, executed in a task when the plugin is loaded
    @classmethod
    async def _main(cls):
        if cls.BACKEND_PROC is not None:
            logger.warning("Wine Cask is already running!")
            return

        logger.info("Starting Wine Cask (the Wine Cellar backend)...")
        cls.BACKEND_PROC = await asyncio.subprocess.create_subprocess_exec(cls.BACKEND_PATH)
        logger.info(f"Wine Cask started with PID {cls.BACKEND_PROC.pid}")

    # Function called first during the unload process, utilize this to handle your plugins being removed
    @classmethod
    async def _unload(cls):
        if cls.BACKEND_PROC is None:
            logger.warning("Wine Cask is not running!")
            return

        logger.info("Terminating Wine Cask (the Wine Cellar backend)...")
        cls.BACKEND_PROC.terminate()

    @classmethod
    async def restart_backend(cls):
        if cls.BACKEND_PROC is not None:
            logger.info("Terminating Wine Cask (the Wine Cellar backend)...")
            cls.BACKEND_PROC.terminate()

        cls.BACKEND_PROC = await asyncio.subprocess.create_subprocess_exec(cls.BACKEND_PATH)

    @classmethod
    async def settings_read(cls):
        logger.info('Reading settings')
        return settings.read()

    @classmethod
    async def settings_commit(cls):
        logger.info('Saving settings')
        return settings.commit()

    @classmethod
    async def settings_getSetting(cls, key: str, defaults):
        logger.info('Get {}'.format(key))
        return settings.getSetting(key, defaults)

    @classmethod
    async def settings_setSetting(cls, key: str, value):
        logger.info('Set {}: {}'.format(key, value))
        return settings.setSetting(key, value)
