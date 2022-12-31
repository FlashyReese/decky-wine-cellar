import logging
import os

logging.basicConfig(filename="/tmp/template.log",
                    format='[Template] %(asctime)s %(levelname)s %(message)s',
                    filemode='w+',
                    force=True)
logger = logging.getLogger()
logger.setLevel(logging.INFO)  # can be changed to logging.DEBUG for debugging issues

compatibility_tools_path = "/home/deck/.steam/root/compatibilitytools.d"

class Plugin:
    in_progress_installs = []

    # Asyncio-compatible long-running code, executed in a task when the plugin is loaded
    async def _main(self):
        logger.info("Hello World!")

    # Function called first during the unload process, utilize this to handle your plugin being removed
    async def _unload(self):
        logger.info("Goodbye World!")
        pass

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
        }

    async def get_installed_compatibility_tools(self):
        entries = os.listdir(compatibility_tools_path)
        existing_installs = [
            self._get_version_from_name(entry, "installed") for entry in entries
        ]

        return existing_installs + self.in_progress_installs

    async def install(self):
        self.in_progress_installs.append({
            "version": "1.0.0",
            "name": "test",
            "status": "in_progress",
        })
