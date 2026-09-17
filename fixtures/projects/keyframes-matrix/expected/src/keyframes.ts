

export const enter = {k:"sw-kf-enter-7nc28"};
export const exit = {k:"sw-kf-tym79"};

export const motionEnabled = Date.now() % 2 === 0;
export const selectedAnimation = motionEnabled ? enter : exit;
