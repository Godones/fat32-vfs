fat32:
	@sudo dd if=/dev/zero of=fat32.img bs=512 count=102400
	@sudo mkfs.vfat -F 32 ./fat32.img