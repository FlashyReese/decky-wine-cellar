import asyncio
import asyncio.subprocess
import logging
import os
import typing

import decky  # type: ignore
from settings import SettingsManager  # type: ignore

PLUGIN_DIR = decky.DECKY_PLUGIN_DIR
PLUGIN_SETTINGS_DIR = decky.DECKY_PLUGIN_SETTINGS_DIR

logger = decky.logger
logger.setLevel(logging.DEBUG)
logger.info("Wine Cellar main.py https://github.com/FlashyReese/decky-wine-cellar")

logger.info('[backend] Settings path: {}'.format(PLUGIN_SETTINGS_DIR))
settings = SettingsManager(name="settings", settings_directory=PLUGIN_SETTINGS_DIR)
settings.read()


class Plugin:
    BACKEND_PATH = f"{PLUGIN_DIR}/bin/backend"
    BACKEND_PROC: typing.Optional[asyncio.subprocess.Process] = None
    BACKEND_WATCH_TASK: typing.Optional[asyncio.Task[None]] = None
    BACKEND_STOPPING = False
    BACKEND_RESTART_DELAY_SECONDS = 2

    @classmethod
    async def _spawn_backend(cls):
        logger.info("Starting Wine Cask (the Wine Cellar backend)...")
        # The backend exits on EOF if this process dies without running _unload.
        cls.BACKEND_PROC = await asyncio.subprocess.create_subprocess_exec(
            cls.BACKEND_PATH,
            stdin=asyncio.subprocess.PIPE,
            env={**os.environ, "WINE_CELLAR_PARENT_PIPE": "1"},
        )
        logger.info(f"Wine Cask started with PID {cls.BACKEND_PROC.pid}")

    @classmethod
    async def _backend_supervisor(cls):
        while not cls.BACKEND_STOPPING:
            try:
                await cls._spawn_backend()
            except Exception:
                logger.exception("Failed to start Wine Cask")
                await asyncio.sleep(cls.BACKEND_RESTART_DELAY_SECONDS)
                continue

            backend_proc = cls.BACKEND_PROC
            if backend_proc is None:
                await asyncio.sleep(cls.BACKEND_RESTART_DELAY_SECONDS)
                continue

            returncode = await backend_proc.wait()
            if cls.BACKEND_PROC is backend_proc:
                cls.BACKEND_PROC = None

            if cls.BACKEND_STOPPING:
                logger.info(f"Wine Cask stopped with exit code {returncode}")
                return

            logger.error(
                f"Wine Cask exited unexpectedly with exit code {returncode}; restarting..."
            )
            await asyncio.sleep(cls.BACKEND_RESTART_DELAY_SECONDS)

    @classmethod
    async def _start_backend_supervisor(cls):
        if cls.BACKEND_WATCH_TASK is not None and not cls.BACKEND_WATCH_TASK.done():
            logger.warning("Wine Cask supervisor is already running!")
            return

        cls.BACKEND_STOPPING = False
        cls.BACKEND_WATCH_TASK = asyncio.create_task(cls._backend_supervisor())

    @classmethod
    async def _stop_backend(cls):
        cls.BACKEND_STOPPING = True

        backend_proc = cls.BACKEND_PROC
        if backend_proc is None:
            logger.warning("Wine Cask is not running!")
        else:
            logger.info("Terminating Wine Cask (the Wine Cellar backend)...")
            try:
                backend_proc.terminate()
            except ProcessLookupError:
                pass

            try:
                await asyncio.wait_for(backend_proc.wait(), timeout=5)
            except asyncio.TimeoutError:
                logger.warning("Wine Cask did not exit after SIGTERM, killing process...")
                try:
                    backend_proc.kill()
                except ProcessLookupError:
                    pass
                await backend_proc.wait()

            cls.BACKEND_PROC = None

        watch_task = cls.BACKEND_WATCH_TASK
        if watch_task is not None and watch_task is not asyncio.current_task():
            try:
                await asyncio.wait_for(watch_task, timeout=6)
            except asyncio.TimeoutError:
                watch_task.cancel()
            except asyncio.CancelledError:
                pass
        cls.BACKEND_WATCH_TASK = None

    @classmethod
    async def _main(cls):
        if cls.BACKEND_WATCH_TASK is not None and not cls.BACKEND_WATCH_TASK.done():
            logger.warning("Wine Cask is already running!")
            return

        await cls._start_backend_supervisor()

    @classmethod
    async def _unload(cls):
        await cls._stop_backend()

    @classmethod
    async def restart_backend(cls):
        await cls._stop_backend()
        await cls._start_backend_supervisor()

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
