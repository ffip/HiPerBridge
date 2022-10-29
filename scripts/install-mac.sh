#/bin/sh
# 通过这个来在构建的同时安装程序
sh ./scripts/build-mac.sh $*
rm -rf "/Applications/HiPer Bridge.app"
cp -rf "./target/HiPer Bridge.app" "/Applications/HiPer Bridge.app"
