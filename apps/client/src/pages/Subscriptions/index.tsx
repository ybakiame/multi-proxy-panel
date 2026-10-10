import { IS_MOBILE } from "@pp/ui";
import Desktop from "./desktop";
import Mobile from "./MobileSubscriptions";

export default function Page() {
  return IS_MOBILE ? <Mobile /> : <Desktop />;
}
