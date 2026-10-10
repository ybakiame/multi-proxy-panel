import { IS_MOBILE } from "@pp/ui";
import Desktop from "./DesktopDashboard";
import Mobile from "./MobileDashboard";

export default function Page() {
  return IS_MOBILE ? <Mobile /> : <Desktop />;
}
