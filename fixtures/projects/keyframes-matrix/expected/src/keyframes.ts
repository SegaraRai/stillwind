

export const enter = {k:"sw-kf-enter-cxv19"};
export const exit = {k:"sw-kf-12rqq"};

export const motionEnabled = Date.now() % 2 === 0;
export const selectedAnimation = motionEnabled ? enter : exit;
