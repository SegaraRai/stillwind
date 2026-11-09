

export const enter = {k:"sw-kf-enter-zhg87"};
export const exit = {k:"sw-kf-074gt"};

export const motionEnabled = Date.now() % 2 === 0;
export const selectedAnimation = motionEnabled ? enter : exit;
