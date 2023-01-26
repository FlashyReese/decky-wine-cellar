import {SidebarNavigation} from 'decky-frontend-lib';

import GitHubReleasesList from "./manage/GitHubReleasesTab";

export default function ManagePage() {

    const pages = [
        {
            title: 'ProtonGE',
            content: <GitHubReleasesList getUrl={'https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases'} />,
            route: '/wine-cellar/protonge',
        },
        {
            title: 'Steam Tinker Launch',
            content: <GitHubReleasesList getUrl={'https://api.github.com/repos/sonic2kk/steamtinkerlaunch/releases'} />,
            route: '/wine-cellar/steamtinkerlaunch',
        },
    ];

    return <SidebarNavigation title="Wine Cellar" showTitle pages={pages} />;
}