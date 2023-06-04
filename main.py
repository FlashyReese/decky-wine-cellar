def get_plugin_dir():
    from pathlib import Path

    return Path(__file__).parent.resolve()


def add_plugin_to_path():
    import sys

    plugin_dir = get_plugin_dir()
    directories = [["./"], ["python"], ["python", "externals"]]
    for dir in directories:
        sys.path.append(str(plugin_dir.joinpath(*dir)))


add_plugin_to_path()

import json
import asyncio
import logging
import os
import io
import aiohttp
import tarfile
from python.externals import vdf
import decky_plugin

logging.basicConfig(filename=decky_plugin.DECKY_PLUGIN_LOG,
                    format='[Wine Cellar] %(asctime)s %(levelname)s %(message)s',
                    filemode='w+',
                    force=True)
logger = logging.getLogger()
logger.setLevel(logging.DEBUG)  # can be changed to logging.DEBUG for debugging issues

# todo: use envars for home path
compatibility_tools_path = decky_plugin.DECKY_USER_HOME + "/.steam/root/compatibilitytools.d"


class Plugin:
    running = True
    in_progress_installs = []

    # Asyncio-compatible long-running code, executed in a task when the plugin is loaded
    async def _main(self):
        logger.info("Hello World!")
        while self.running:
            if len(self.in_progress_installs) == 0:
                logger.debug("No pending installs. Sleeping.")
                await asyncio.sleep(5)
                pass

            for install in self.in_progress_installs:
                if install['status'] == "in_queue":
                    install['status'] = "in_progress"
                    async with aiohttp.ClientSession() as session:
                        async with session.get(install['url'], ssl=False) as resp:
                            if resp.status == 200:
                                path = compatibility_tools_path + "/"
                                b = io.BytesIO()
                                downloaded_size = 0
                                async for chunk in resp.content.iter_chunks():
                                    chunk_bytes = chunk[0]
                                    downloaded_size += len(chunk_bytes)
                                    install['current_size'] = downloaded_size
                                    b.write(chunk_bytes)
                                b.seek(0)
                                tar = tarfile.open(fileobj=b, mode='r:gz')
                                tar.extractall(path)
                                install['status'] = "completed"
                                logger.info("Download completed for: " + json.dumps(install))

    # Function called first during the unload process, utilize this to handle your plugin being removed
    async def _unload(self):
        self.running = False
        logger.info("Goodbye World!")
        pass

    def _get_compat_tools():
        entries = os.listdir(compatibility_tools_path)
        result = []
        for entry in entries:
            compat_tool_vdf_path = compatibility_tools_path + '/' + entry + '/compatibilitytool.vdf'
            version_path = compatibility_tools_path + '/' + entry + '/version'
            if os.path.exists(compat_tool_vdf_path):
                d = vdf.load(open(compat_tool_vdf_path))
                internal_name = list(d['compatibilitytools']['compat_tools'].keys())[0]
                version = open(version_path).read().split(" ")[0].strip() if os.path.exists(version_path) else None
                result.append({
                    "internal": internal_name,
                    "display": d['compatibilitytools']['compat_tools'][internal_name]['display_name'],
                    "version": version
                })
        return result

    # These methods below were pulled from DeckyProtonManager, todo: This doesn't detect Steam Tinker Launch
    def _get_version_from_name(name, status):
        path = compatibility_tools_path + "/" + name + "/version"

        version_string = None

        with open(path) as version:
            version_string = version.read()

        split_version_string = version_string.split(" ")

        return {
            "version": split_version_string[0].strip(),
            "name": split_version_string[1].strip(),
            "status": status,
            "progress": 100,
        }

    async def get_installed_compatibility_tools(self):
        entries = os.listdir(compatibility_tools_path)
        existing_installs = [
            self._get_version_from_name(entry, "installed") for entry in entries
        ]

        return existing_installs + self.in_progress_installs

    async def add_to_queue(self, release):
        for asset in release['assets']:
            if asset['content_type'] == 'application/gzip':
                url = asset['browser_download_url']
                size = asset['size']
                break
        else:
            logger.error("No ZIP content founded in " + release['tag_name'])
            return

        action = {
            "name": release['tag_name'],
            "status": "in_queue",
            "url": url,
            "size": size,
            "current_size": 0
        }
        logger.info("Adding to queue: " + json.dumps(action))
        self.in_progress_installs.append(action)

    async def get_release_installation_progress(self, release):
        for installs in self.in_progress_installs:
            if installs['name'] == release['tag_name'] & installs['status'] == "in_progress":
                return installs['current_size'] / installs['size']
        return 0
