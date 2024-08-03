import dayjs, { Dayjs } from 'dayjs';

import localizedFormat from 'dayjs/plugin/localizedFormat';
import advancedFormat from 'dayjs/plugin/advancedFormat';
import utc from 'dayjs/plugin/utc';
import timezone from 'dayjs/plugin/timezone';

dayjs.extend(localizedFormat);
dayjs.extend(advancedFormat);
dayjs.extend(timezone);
dayjs.extend(utc);

window.dayjs = dayjs;
export { dayjs, type Dayjs };
